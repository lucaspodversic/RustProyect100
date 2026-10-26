//! # Renombrador masivo — lógica pura
//!
//! Este módulo contiene TODA la lógica del renombrador separada de la CLI,
//! para poder testearla sin tocar `std::env::args` ni la terminal.
//!
//! Flujo general:
//! 1. [`planificar`] recibe la lista de nombres de un directorio, una regex y
//!    un patrón de reemplazo, y devuelve un [`Plan`] con los cambios propuestos.
//! 2. El plan se valida: detecta **colisiones** (dos archivos que terminarían
//!    con el mismo nombre, o un destino que ya existe y no se renombra).
//! 3. Si el usuario no pidió `--dry-run`, [`aplicar`] ejecuta los renombres
//!    en disco usando nombres temporales para soportar intercambios (a→b, b→a).

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Un renombre individual propuesto: nombre original → nombre nuevo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renombre {
    /// Nombre de archivo actual (sin directorio).
    pub desde: String,
    /// Nombre de archivo resultante tras aplicar la regex.
    pub hacia: String,
}

/// Problemas que impiden aplicar un plan de forma segura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conflicto {
    /// Dos o más archivos terminarían con el mismo nombre destino.
    DestinoDuplicado { destino: String, origenes: Vec<String> },
    /// El destino ya existe en el directorio y ese archivo NO se renombra,
    /// así que lo pisaríamos.
    DestinoExistente { destino: String, origen: String },
    /// El nombre resultante es inválido (vacío o contiene un separador de ruta).
    NombreInvalido { origen: String, destino: String },
}

/// Resultado de planificar: cambios + conflictos encontrados.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Renombres efectivos (solo los que cambian el nombre).
    pub cambios: Vec<Renombre>,
    /// Conflictos detectados; si no está vacío, NO se debe aplicar.
    pub conflictos: Vec<Conflicto>,
}

impl Plan {
    /// `true` si el plan puede aplicarse sin riesgo.
    pub fn es_seguro(&self) -> bool {
        self.conflictos.is_empty()
    }
}

/// Opciones de planificación.
#[derive(Debug, Clone, Default)]
pub struct Opciones {
    /// Si es `true`, reemplaza todas las coincidencias; si no, solo la primera.
    pub todas: bool,
    /// Incluir archivos ocultos (los que empiezan con `.`).
    pub incluir_ocultos: bool,
}

/// Construye el plan de renombres para una lista de nombres.
///
/// * `nombres` — nombres de archivos existentes en el directorio.
/// * `re` — regex que debe coincidir para que un archivo se renombre.
/// * `reemplazo` — patrón de reemplazo; admite grupos `$1`, `${nombre}`.
///
/// Los archivos que no coinciden o cuyo nombre no cambia se ignoran.
pub fn planificar(nombres: &[String], re: &Regex, reemplazo: &str, opts: &Opciones) -> Plan {
    let mut plan = Plan::default();

    // 1) Calculamos el nombre nuevo de cada archivo que coincide.
    for nombre in nombres {
        // Saltamos ocultos salvo que se pidan explícitamente.
        if !opts.incluir_ocultos && nombre.starts_with('.') {
            continue;
        }
        if !re.is_match(nombre) {
            continue;
        }
        // `replace_all` vs `replace`: decide si se reemplazan todas las apariciones.
        let nuevo = if opts.todas {
            re.replace_all(nombre, reemplazo).into_owned()
        } else {
            re.replace(nombre, reemplazo).into_owned()
        };
        if nuevo == *nombre {
            continue; // nada que hacer
        }
        // Un nombre vacío o con '/' o '\' escaparía del directorio: inválido.
        if nuevo.is_empty() || nuevo.contains('/') || nuevo.contains('\\') || nuevo == "." || nuevo == ".." {
            plan.conflictos.push(Conflicto::NombreInvalido { origen: nombre.clone(), destino: nuevo });
            continue;
        }
        plan.cambios.push(Renombre { desde: nombre.clone(), hacia: nuevo });
    }

    // 2) Detectamos destinos duplicados agrupando por nombre destino.
    let mut por_destino: HashMap<&str, Vec<String>> = HashMap::new();
    for c in &plan.cambios {
        por_destino.entry(c.hacia.as_str()).or_default().push(c.desde.clone());
    }
    let mut duplicados: Vec<Conflicto> = por_destino
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(d, mut v)| {
            v.sort();
            Conflicto::DestinoDuplicado { destino: d.to_string(), origenes: v }
        })
        .collect();
    // Orden determinista para que la salida y los tests sean estables.
    duplicados.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    plan.conflictos.extend(duplicados);

    // 3) Destino que ya existe y que NO se mueve de lugar → lo pisaríamos.
    let que_se_mueven: HashSet<&str> = plan.cambios.iter().map(|c| c.desde.as_str()).collect();
    let existentes: HashSet<&str> = nombres.iter().map(|s| s.as_str()).collect();
    for c in &plan.cambios {
        if existentes.contains(c.hacia.as_str()) && !que_se_mueven.contains(c.hacia.as_str()) {
            plan.conflictos.push(Conflicto::DestinoExistente { destino: c.hacia.clone(), origen: c.desde.clone() });
        }
    }
    plan
}

/// Lista los nombres de archivos (no directorios) de `dir`, ordenados.
pub fn listar_archivos(dir: &Path) -> io::Result<Vec<String>> {
    let mut v = Vec::new();
    for entrada in fs::read_dir(dir)? {
        let entrada = entrada?;
        if entrada.file_type()?.is_file() {
            // `to_string_lossy` evita fallar con nombres no-UTF8 (se muestran con �).
            v.push(entrada.file_name().to_string_lossy().into_owned());
        }
    }
    v.sort();
    Ok(v)
}

/// Aplica el plan en disco en dos fases para soportar ciclos (a→b, b→a):
/// primero cada origen pasa a un nombre temporal único, luego al destino final.
///
/// Devuelve error si el plan tiene conflictos.
pub fn aplicar(dir: &Path, plan: &Plan) -> io::Result<usize> {
    if !plan.es_seguro() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "el plan tiene conflictos; no se aplica"));
    }
    // Fase 1: origen → temporal.
    let mut temporales: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(plan.cambios.len());
    for (i, c) in plan.cambios.iter().enumerate() {
        let tmp = dir.join(format!(".renombrador-tmp-{}-{i}", std::process::id()));
        fs::rename(dir.join(&c.desde), &tmp)?;
        temporales.push((tmp, dir.join(&c.hacia)));
    }
    // Fase 2: temporal → destino.
    for (tmp, destino) in &temporales {
        fs::rename(tmp, destino)?;
    }
    Ok(temporales.len())
}

/// Formatea el plan como tabla legible para la previsualización.
pub fn formatear_plan(plan: &Plan) -> String {
    let mut s = String::new();
    // Ancho de la primera columna para alinear las flechas.
    let ancho = plan.cambios.iter().map(|c| c.desde.chars().count()).max().unwrap_or(0);
    for c in &plan.cambios {
        s.push_str(&format!("  {:<ancho$}  →  {}\n", c.desde, c.hacia, ancho = ancho));
    }
    for k in &plan.conflictos {
        s.push_str(&format!("  ⚠ {}\n", describir_conflicto(k)));
    }
    s
}

/// Texto en español para cada conflicto.
pub fn describir_conflicto(c: &Conflicto) -> String {
    match c {
        Conflicto::DestinoDuplicado { destino, origenes } => {
            format!("'{destino}' sería destino de varios archivos: {}", origenes.join(", "))
        }
        Conflicto::DestinoExistente { destino, origen } => {
            format!("'{origen}' pisaría a '{destino}', que ya existe")
        }
        Conflicto::NombreInvalido { origen, destino } => {
            format!("'{origen}' produciría un nombre inválido: '{destino}'")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn renombra_con_grupos() {
        let re = Regex::new(r"^IMG_(\d+)\.jpg$").unwrap();
        let p = planificar(&v(&["IMG_001.jpg", "otro.txt"]), &re, "foto-$1.jpg", &Opciones::default());
        assert_eq!(p.cambios, vec![Renombre { desde: "IMG_001.jpg".into(), hacia: "foto-001.jpg".into() }]);
        assert!(p.es_seguro());
    }

    #[test]
    fn detecta_destino_duplicado() {
        let re = Regex::new(r"\d+").unwrap();
        let p = planificar(&v(&["a1.txt", "a2.txt"]), &re, "X", &Opciones::default());
        assert!(matches!(p.conflictos[0], Conflicto::DestinoDuplicado { .. }));
    }

    #[test]
    fn detecta_destino_existente() {
        let re = Regex::new(r"^b").unwrap();
        let p = planificar(&v(&["a.txt", "b.txt"]), &re, "a", &Opciones::default());
        assert!(matches!(p.conflictos[0], Conflicto::DestinoExistente { .. }));
    }

    #[test]
    fn rechaza_separadores() {
        let re = Regex::new(r"x").unwrap();
        let p = planificar(&v(&["x.txt"]), &re, "../y", &Opciones::default());
        assert!(matches!(p.conflictos[0], Conflicto::NombreInvalido { .. }));
    }

    #[test]
    fn ignora_ocultos_por_defecto() {
        let re = Regex::new(r"a").unwrap();
        let p = planificar(&v(&[".a"]), &re, "b", &Opciones::default());
        assert!(p.cambios.is_empty());
        let p = planificar(&v(&[".a"]), &re, "b", &Opciones { incluir_ocultos: true, ..Default::default() });
        assert_eq!(p.cambios.len(), 1);
    }

    #[test]
    fn todas_vs_primera() {
        let re = Regex::new(r"a").unwrap();
        let p1 = planificar(&v(&["aaa"]), &re, "b", &Opciones::default());
        assert_eq!(p1.cambios[0].hacia, "baa");
        let p2 = planificar(&v(&["aaa"]), &re, "b", &Opciones { todas: true, ..Default::default() });
        assert_eq!(p2.cambios[0].hacia, "bbb");
    }

    #[test]
    fn aplica_intercambio_en_disco() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), "A").unwrap();
        fs::write(dir.path().join("b"), "B").unwrap();
        // Intercambio a↔b: regex que mapea cada uno al otro.
        let plan = Plan {
            cambios: vec![
                Renombre { desde: "a".into(), hacia: "b".into() },
                Renombre { desde: "b".into(), hacia: "a".into() },
            ],
            conflictos: vec![],
        };
        assert_eq!(aplicar(dir.path(), &plan).unwrap(), 2);
        assert_eq!(fs::read_to_string(dir.path().join("a")).unwrap(), "B");
        assert_eq!(fs::read_to_string(dir.path().join("b")).unwrap(), "A");
    }

    #[test]
    fn no_aplica_si_hay_conflictos() {
        let dir = tempfile::tempdir().unwrap();
        let plan = Plan { cambios: vec![], conflictos: vec![Conflicto::NombreInvalido { origen: "a".into(), destino: "".into() }] };
        assert!(aplicar(dir.path(), &plan).is_err());
    }

    #[test]
    fn listar_y_formatear() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("z.log"), "").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        assert_eq!(listar_archivos(dir.path()).unwrap(), v(&["z.log"]));
        let re = Regex::new(r"\.log$").unwrap();
        let p = planificar(&v(&["z.log"]), &re, ".txt", &Opciones::default());
        assert!(formatear_plan(&p).contains("z.log  →  z.txt"));
    }
}
