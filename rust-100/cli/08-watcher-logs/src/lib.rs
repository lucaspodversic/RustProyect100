//! # Watcher de logs — lógica
//!
//! Funciona como `tail -F` + disparador:
//! - [`Seguidor`] recuerda el offset leído del archivo y en cada [`Seguidor::leer_nuevas`]
//!   devuelve solo las líneas completas nuevas. Detecta **truncado** (el archivo
//!   se achicó) y **rotación** (cambió el inodo en Unix) y vuelve a empezar.
//! - [`Disparador`] decide si una línea dispara la acción: regex + *cooldown*
//!   (no disparar más de una vez cada N segundos) + máximo de disparos.
//! - [`Ejecutor`] es un trait: el real lanza `sh -c` / `cmd /C` con la línea en
//!   la variable de entorno `WATCH_LINE`; en los tests usamos uno que registra.
//!
//! Se usa *polling* (sin inotify) para ser portable y sin dependencias.

use regex::Regex;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::{Duration, Instant};

/// Sigue un archivo que crece.
pub struct Seguidor {
    ruta: PathBuf,
    offset: u64,
    /// Resto de una línea incompleta (sin `\n` todavía).
    pendiente: String,
    /// Identidad del archivo (inodo en Unix) para detectar rotación.
    identidad: Option<u64>,
}

/// Identidad del archivo: inodo en Unix; en otros SO no detectamos rotación por inodo.
fn identidad(meta: &std::fs::Metadata) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(meta.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        None
    }
}

impl Seguidor {
    /// Crea el seguidor. Con `desde_inicio = false` empieza al final (como `tail -f`).
    pub fn new(ruta: &Path, desde_inicio: bool) -> io::Result<Self> {
        let (offset, id) = match std::fs::metadata(ruta) {
            Ok(m) => (if desde_inicio { 0 } else { m.len() }, identidad(&m)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (0, None), // puede aparecer después
            Err(e) => return Err(e),
        };
        Ok(Seguidor { ruta: ruta.to_path_buf(), offset, pendiente: String::new(), identidad: id })
    }

    /// Lee las líneas completas agregadas desde la última llamada.
    pub fn leer_nuevas(&mut self) -> io::Result<Vec<String>> {
        let meta = match std::fs::metadata(&self.ruta) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]), // rotado y aún no recreado
            Err(e) => return Err(e),
        };
        let id = identidad(&meta);
        // Rotación (otro inodo) o truncado (más chico que lo leído): reiniciamos.
        if id != self.identidad || meta.len() < self.offset {
            self.offset = 0;
            self.pendiente.clear();
            self.identidad = id;
        }
        if meta.len() == self.offset {
            return Ok(vec![]);
        }
        let mut f = File::open(&self.ruta)?;
        f.seek(SeekFrom::Start(self.offset))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        self.offset += buf.len() as u64;
        self.pendiente.push_str(&String::from_utf8_lossy(&buf));
        // Separamos líneas completas; lo que queda tras el último '\n' sigue pendiente.
        let mut lineas = Vec::new();
        while let Some(p) = self.pendiente.find('\n') {
            let l: String = self.pendiente.drain(..=p).collect();
            lineas.push(l.trim_end_matches(['\n', '\r']).to_string());
        }
        Ok(lineas)
    }
}

/// Reglas para decidir cuándo disparar.
pub struct Disparador {
    pub patron: Regex,
    pub cooldown: Duration,
    pub max_disparos: Option<u64>,
    ultimo: Option<Instant>,
    pub disparos: u64,
}

impl Disparador {
    pub fn new(patron: Regex, cooldown: Duration, max_disparos: Option<u64>) -> Self {
        Disparador { patron, cooldown, max_disparos, ultimo: None, disparos: 0 }
    }

    /// ¿Esta línea dispara *ahora*? Recibe `ahora` como parámetro para testear el tiempo.
    pub fn evaluar(&mut self, linea: &str, ahora: Instant) -> bool {
        if !self.patron.is_match(linea) || self.agotado() {
            return false;
        }
        if let Some(u) = self.ultimo {
            if ahora.duration_since(u) < self.cooldown {
                return false; // dentro del cooldown: se ignora
            }
        }
        self.ultimo = Some(ahora);
        self.disparos += 1;
        true
    }

    /// `true` si ya se alcanzó el máximo de disparos.
    pub fn agotado(&self) -> bool {
        self.max_disparos.is_some_and(|m| self.disparos >= m)
    }
}

/// Algo capaz de ejecutar la acción.
pub trait Ejecutor {
    fn ejecutar(&mut self, comando: &str, linea: &str) -> io::Result<ExitStatus>;
}

/// Ejecutor real usando la shell del sistema.
pub struct EjecutorShell;

impl Ejecutor for EjecutorShell {
    fn ejecutar(&mut self, comando: &str, linea: &str) -> io::Result<ExitStatus> {
        // La línea va por variable de entorno (NO interpolada en el comando)
        // para evitar inyección de comandos con contenido del log.
        #[cfg(unix)]
        let mut c = {
            let mut c = Command::new("sh");
            c.arg("-c").arg(comando);
            c
        };
        #[cfg(windows)]
        let mut c = {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(comando);
            c
        };
        c.env("WATCH_LINE", linea).status()
    }
}

/// Un paso del bucle: lee nuevas líneas y dispara lo que corresponda.
/// Devuelve cuántas veces se ejecutó el comando.
pub fn paso<E: Ejecutor>(seg: &mut Seguidor, disp: &mut Disparador, ej: &mut E, comando: &str, ahora: Instant) -> io::Result<u64> {
    let mut n = 0;
    for l in seg.leer_nuevas()? {
        if disp.evaluar(&l, ahora) {
            ej.ejecutar(comando, &l)?;
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct Registro(Vec<(String, String)>);
    impl Ejecutor for Registro {
        fn ejecutar(&mut self, c: &str, l: &str) -> io::Result<ExitStatus> {
            self.0.push((c.into(), l.into()));
            EjecutorShell.ejecutar("true", l) // obtener un ExitStatus válido
        }
    }

    fn agregar(p: &Path, s: &str) {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p).unwrap();
        f.write_all(s.as_bytes()).unwrap();
    }

    #[test]
    fn lee_solo_lineas_nuevas_y_completas() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("app.log");
        agregar(&p, "vieja\n");
        let mut s = Seguidor::new(&p, false).unwrap();
        assert!(s.leer_nuevas().unwrap().is_empty());
        agregar(&p, "uno\ndo");
        assert_eq!(s.leer_nuevas().unwrap(), vec!["uno"]);
        agregar(&p, "s\r\n");
        assert_eq!(s.leer_nuevas().unwrap(), vec!["dos"]);
    }

    #[test]
    fn detecta_truncado_y_rotacion() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("app.log");
        agregar(&p, "aaaaaaaaaa\n");
        let mut s = Seguidor::new(&p, true).unwrap();
        assert_eq!(s.leer_nuevas().unwrap().len(), 1);
        std::fs::write(&p, "b\n").unwrap(); // truncado
        assert_eq!(s.leer_nuevas().unwrap(), vec!["b"]);
        // Rotación: renombrar y crear uno nuevo.
        std::fs::rename(&p, d.path().join("app.log.1")).unwrap();
        assert!(s.leer_nuevas().unwrap().is_empty());
        agregar(&p, "nuevo archivo de log, bastante largo\n");
        assert_eq!(s.leer_nuevas().unwrap(), vec!["nuevo archivo de log, bastante largo"]);
    }

    #[test]
    fn archivo_inexistente_al_inicio() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("todavia-no.log");
        let mut s = Seguidor::new(&p, false).unwrap();
        assert!(s.leer_nuevas().unwrap().is_empty());
        agregar(&p, "hola\n");
        assert_eq!(s.leer_nuevas().unwrap(), vec!["hola"]);
    }

    #[test]
    fn cooldown_y_maximo() {
        let mut d = Disparador::new(Regex::new("ERROR").unwrap(), Duration::from_secs(10), Some(2));
        let t = Instant::now();
        assert!(d.evaluar("ERROR 1", t));
        assert!(!d.evaluar("ERROR 2", t + Duration::from_secs(5))); // cooldown
        assert!(!d.evaluar("INFO", t + Duration::from_secs(20)));
        assert!(d.evaluar("ERROR 3", t + Duration::from_secs(20)));
        assert!(!d.evaluar("ERROR 4", t + Duration::from_secs(60))); // máximo
        assert!(d.agotado());
    }

    #[test]
    fn paso_completo_con_registro() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("x.log");
        let mut s = Seguidor::new(&p, false).unwrap();
        let mut disp = Disparador::new(Regex::new("panic").unwrap(), Duration::ZERO, None);
        let mut r = Registro(vec![]);
        agregar(&p, "ok\nthread panic at x\nok\npanic again\n");
        let n = paso(&mut s, &mut disp, &mut r, "notificar", Instant::now()).unwrap();
        assert_eq!(n, 2);
        assert_eq!(r.0[1].1, "panic again");
    }

    #[cfg(unix)]
    #[test]
    fn ejecutor_shell_pasa_la_linea_por_env() {
        let d = tempfile::tempdir().unwrap();
        let out = d.path().join("out.txt");
        let cmd = format!("printf '%s' \"$WATCH_LINE\" > {}", out.display());
        // Contenido malicioso: NO debe ejecutarse como comando.
        let st = EjecutorShell.ejecutar(&cmd, "ERROR; rm -rf /tmp/nada").unwrap();
        assert!(st.success());
        assert_eq!(std::fs::read_to_string(out).unwrap(), "ERROR; rm -rf /tmp/nada");
    }
}
