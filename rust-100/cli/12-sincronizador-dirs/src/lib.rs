//! # Sincronizador de directorios — lógica
//!
//! Sincroniza **origen → destino** en dos fases, como `rsync -a --delete` simplificado:
//!
//! 1. [`planificar`]: recorre ambos árboles, compara cada ruta relativa y produce
//!    un `Vec<Accion>` (crear dir, copiar nuevo, actualizar, borrar sobrante).
//!    No toca el disco → sirve para `--simular` (dry-run).
//! 2. [`ejecutar`]: aplica las acciones en orden seguro (dirs antes que archivos,
//!    borrados de lo más profundo a lo más superficial).
//!
//! ## ¿Cuándo un archivo "cambió"?
//! - Modo rápido (por defecto): tamaño distinto **o** mtime distinto (con tolerancia
//!   de 2 s, por sistemas de archivos como FAT que redondean).
//! - Modo `--checksum`: compara hash BLAKE3 del contenido (más lento, exacto).
//!
//! Tras copiar, se replica el mtime del origen en el destino: así la siguiente
//! corrida no detecta cambios (sincronización idempotente).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

/// Metadatos mínimos de una entrada del árbol.
#[derive(Debug, Clone)]
struct Entrada {
    es_dir: bool,
    tam: u64,
    mtime: SystemTime,
}

/// Una operación a realizar en el destino (rutas relativas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accion {
    CrearDir(PathBuf),
    Copiar(PathBuf),
    Actualizar(PathBuf),
    Borrar(PathBuf),
    BorrarDir(PathBuf),
}

/// Opciones de sincronización.
#[derive(Debug, Clone, Default)]
pub struct Opciones {
    /// Borrar en destino lo que no exista en origen.
    pub borrar: bool,
    /// Comparar por contenido (hash) en vez de tamaño+mtime.
    pub checksum: bool,
    /// Patrones simples de exclusión: si algún componente de la ruta coincide exacto
    /// (ej. `.git`, `node_modules`) o termina en el sufijo `*.ext`.
    pub excluir: Vec<String>,
}

fn excluido(rel: &Path, pats: &[String]) -> bool {
    rel.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        pats.iter().any(|p| match p.strip_prefix('*') {
            Some(suf) => s.ends_with(suf),
            None => s == p.as_str(),
        })
    })
}

/// Mapa ruta relativa → metadatos (BTreeMap: orden determinista, padres antes que hijos).
fn indexar(raiz: &Path, op: &Opciones) -> io::Result<BTreeMap<PathBuf, Entrada>> {
    let mut m = BTreeMap::new();
    if !raiz.exists() {
        return Ok(m);
    }
    for e in WalkDir::new(raiz).min_depth(1) {
        let e = e.map_err(io::Error::other)?;
        let rel = e.path().strip_prefix(raiz).expect("dentro de la raíz").to_path_buf();
        if excluido(&rel, &op.excluir) {
            continue;
        }
        let md = e.metadata().map_err(io::Error::other)?;
        // Los symlinks se tratan como archivos normales siguiendo el contenido (limitación).
        m.insert(rel, Entrada { es_dir: md.is_dir(), tam: md.len(), mtime: md.modified()? });
    }
    Ok(m)
}

fn hash(p: &Path) -> io::Result<blake3::Hash> {
    let mut h = blake3::Hasher::new();
    io::copy(&mut fs::File::open(p)?, &mut h)?;
    Ok(h.finalize())
}

fn mtime_distinto(a: SystemTime, b: SystemTime) -> bool {
    let d = a.duration_since(b).or_else(|_| b.duration_since(a)).unwrap_or_default();
    d > Duration::from_secs(2)
}

/// Calcula las acciones necesarias para que `destino` refleje `origen`.
pub fn planificar(origen: &Path, destino: &Path, op: &Opciones) -> io::Result<Vec<Accion>> {
    let o = indexar(origen, op)?;
    let d = indexar(destino, op)?;
    let mut acc = Vec::new();
    for (rel, eo) in &o {
        match d.get(rel) {
            None if eo.es_dir => acc.push(Accion::CrearDir(rel.clone())),
            None => acc.push(Accion::Copiar(rel.clone())),
            Some(ed) if ed.es_dir != eo.es_dir => {
                // Cambió de tipo (archivo ↔ dir): borrar y recrear.
                acc.push(if ed.es_dir { Accion::BorrarDir(rel.clone()) } else { Accion::Borrar(rel.clone()) });
                acc.push(if eo.es_dir { Accion::CrearDir(rel.clone()) } else { Accion::Copiar(rel.clone()) });
            }
            Some(_) if eo.es_dir => {}
            Some(ed) => {
                let cambio = if op.checksum {
                    hash(&origen.join(rel))? != hash(&destino.join(rel))?
                } else {
                    ed.tam != eo.tam || mtime_distinto(ed.mtime, eo.mtime)
                };
                if cambio {
                    acc.push(Accion::Actualizar(rel.clone()));
                }
            }
        }
    }
    if op.borrar {
        // Recorremos al revés para borrar hijos antes que padres.
        for (rel, ed) in d.iter().rev() {
            if !o.contains_key(rel) {
                acc.push(if ed.es_dir { Accion::BorrarDir(rel.clone()) } else { Accion::Borrar(rel.clone()) });
            }
        }
    }
    Ok(acc)
}

/// Resumen de la ejecución.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Resumen {
    pub copiados: usize,
    pub actualizados: usize,
    pub borrados: usize,
    pub dirs_creados: usize,
    pub bytes: u64,
}

/// Copia un archivo preservando mtime (para idempotencia). Escribe a `.rsincro.tmp`
/// y renombra: si se corta a mitad, el destino nunca queda con un archivo a medias.
fn copiar(src: &Path, dst: &Path) -> io::Result<u64> {
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p)?;
    }
    let tmp = dst.with_extension("rsincro.tmp");
    let n = fs::copy(src, &tmp)?;
    let mt = fs::metadata(src)?.modified()?;
    fs::File::options().write(true).open(&tmp)?.set_modified(mt)?;
    fs::rename(&tmp, dst)?;
    Ok(n)
}

/// Ejecuta un plan. Los borrados de un plan con cambio de tipo ya vienen ordenados.
pub fn ejecutar(origen: &Path, destino: &Path, plan: &[Accion]) -> io::Result<Resumen> {
    let mut r = Resumen::default();
    fs::create_dir_all(destino)?;
    for a in plan {
        match a {
            Accion::CrearDir(p) => {
                fs::create_dir_all(destino.join(p))?;
                r.dirs_creados += 1;
            }
            Accion::Copiar(p) => {
                r.bytes += copiar(&origen.join(p), &destino.join(p))?;
                r.copiados += 1;
            }
            Accion::Actualizar(p) => {
                r.bytes += copiar(&origen.join(p), &destino.join(p))?;
                r.actualizados += 1;
            }
            Accion::Borrar(p) => {
                fs::remove_file(destino.join(p))?;
                r.borrados += 1;
            }
            Accion::BorrarDir(p) => {
                // remove_dir_all: el dir puede contener cosas excluidas del índice.
                fs::remove_dir_all(destino.join(p))?;
                r.borrados += 1;
            }
        }
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn escribir(p: &Path, s: &str) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, s).unwrap();
    }

    #[test]
    fn copia_inicial_y_luego_idempotente() {
        let (o, d) = (tempdir().unwrap(), tempdir().unwrap());
        escribir(&o.path().join("a.txt"), "hola");
        escribir(&o.path().join("sub/b.txt"), "mundo");
        let op = Opciones::default();
        let plan = planificar(o.path(), d.path(), &op).unwrap();
        assert_eq!(plan.len(), 3); // sub/, a.txt, sub/b.txt
        let r = ejecutar(o.path(), d.path(), &plan).unwrap();
        assert_eq!((r.copiados, r.dirs_creados, r.bytes), (2, 1, 9));
        assert_eq!(fs::read_to_string(d.path().join("sub/b.txt")).unwrap(), "mundo");
        // Segunda corrida: nada que hacer gracias al mtime preservado.
        assert!(planificar(o.path(), d.path(), &op).unwrap().is_empty());
    }

    #[test]
    fn detecta_cambios_por_tamano() {
        let (o, d) = (tempdir().unwrap(), tempdir().unwrap());
        escribir(&o.path().join("a.txt"), "v1");
        let op = Opciones::default();
        ejecutar(o.path(), d.path(), &planificar(o.path(), d.path(), &op).unwrap()).unwrap();
        escribir(&o.path().join("a.txt"), "version 2");
        let plan = planificar(o.path(), d.path(), &op).unwrap();
        assert_eq!(plan, vec![Accion::Actualizar("a.txt".into())]);
    }

    #[test]
    fn checksum_detecta_mismo_tamano_mismo_mtime() {
        let (o, d) = (tempdir().unwrap(), tempdir().unwrap());
        escribir(&o.path().join("a.txt"), "AAAA");
        escribir(&d.path().join("a.txt"), "BBBB");
        // Igualamos mtimes para engañar al modo rápido.
        let mt = fs::metadata(o.path().join("a.txt")).unwrap().modified().unwrap();
        fs::File::options().write(true).open(d.path().join("a.txt")).unwrap().set_modified(mt).unwrap();
        assert!(planificar(o.path(), d.path(), &Opciones::default()).unwrap().is_empty());
        let op = Opciones { checksum: true, ..Default::default() };
        assert_eq!(planificar(o.path(), d.path(), &op).unwrap().len(), 1);
    }

    #[test]
    fn borrar_sobrantes_solo_si_se_pide() {
        let (o, d) = (tempdir().unwrap(), tempdir().unwrap());
        escribir(&d.path().join("viejo/x.txt"), "x");
        assert!(planificar(o.path(), d.path(), &Opciones::default()).unwrap().is_empty());
        let op = Opciones { borrar: true, ..Default::default() };
        let plan = planificar(o.path(), d.path(), &op).unwrap();
        // Hijo antes que padre.
        assert_eq!(plan, vec![Accion::Borrar("viejo/x.txt".into()), Accion::BorrarDir("viejo".into())]);
        ejecutar(o.path(), d.path(), &plan).unwrap();
        assert!(!d.path().join("viejo").exists());
    }

    #[test]
    fn exclusiones() {
        let (o, d) = (tempdir().unwrap(), tempdir().unwrap());
        escribir(&o.path().join(".git/config"), "x");
        escribir(&o.path().join("a.log"), "x");
        escribir(&o.path().join("a.rs"), "x");
        let op = Opciones { excluir: vec![".git".into(), "*.log".into()], ..Default::default() };
        assert_eq!(planificar(o.path(), d.path(), &op).unwrap(), vec![Accion::Copiar("a.rs".into())]);
    }

    #[test]
    fn cambio_de_tipo_archivo_a_dir() {
        let (o, d) = (tempdir().unwrap(), tempdir().unwrap());
        escribir(&o.path().join("cosa/dentro.txt"), "x");
        escribir(&d.path().join("cosa"), "era archivo");
        let plan = planificar(o.path(), d.path(), &Opciones::default()).unwrap();
        ejecutar(o.path(), d.path(), &plan).unwrap();
        assert!(d.path().join("cosa/dentro.txt").is_file());
    }
}
