//! # Limpieza de disco segura — lógica
//!
//! Encuentra espacio recuperable y lo libera con **varias capas de seguridad**:
//!
//! 1. **Solo categorías conocidas** ([`Categoria`]): artefactos de build que se
//!    regeneran (`target/` junto a `Cargo.toml`, `node_modules/` junto a
//!    `package.json`, `__pycache__`), cachés de usuario (`~/.cache`, cachés de
//!    cargo/npm/pip) y archivos temporales viejos.
//! 2. **Antigüedad mínima**: solo se proponen cosas sin modificar hace N días
//!    (se mira el mtime más reciente *dentro* del directorio).
//! 3. **Validación de ruta** ([`ruta_segura`]): canonicaliza, rechaza symlinks,
//!    exige estar *estrictamente dentro* de una raíz permitida y nunca toca
//!    `/`, el home ni la raíz misma.
//! 4. **Simulación por defecto**: el binario solo lista; hay que pasar `--borrar`
//!    o `--papelera DIR` (mover en vez de borrar, recuperable).
//! 5. Re-validación justo antes de borrar (por si algo cambió entre escaneo y acción).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

/// Tipos de basura reconocida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Categoria {
    BuildRust,
    NodeModules,
    CachePython,
    CacheUsuario,
    Temporal,
}

/// Algo que se podría borrar.
#[derive(Debug, Clone)]
pub struct Candidato {
    pub ruta: PathBuf,
    pub categoria: Categoria,
    pub bytes: u64,
    /// Tiempo desde la última modificación de cualquier cosa adentro.
    pub inactivo: Duration,
}

/// Tamaño total y mtime más reciente de un árbol (sin seguir symlinks).
pub fn medir(ruta: &Path) -> (u64, SystemTime) {
    let mut bytes = 0;
    let mut reciente = SystemTime::UNIX_EPOCH;
    for e in WalkDir::new(ruta).follow_links(false).into_iter().flatten() {
        if let Ok(md) = e.metadata() {
            if md.is_file() {
                bytes += md.len();
            }
            if let Ok(m) = md.modified() {
                reciente = reciente.max(m);
            }
        }
    }
    (bytes, reciente)
}

fn candidato(ruta: PathBuf, categoria: Categoria, ahora: SystemTime) -> Candidato {
    let (bytes, m) = medir(&ruta);
    Candidato { ruta, categoria, bytes, inactivo: ahora.duration_since(m).unwrap_or_default() }
}

/// Busca artefactos de proyectos bajo `raiz`. No desciende dentro de un artefacto
/// encontrado (un `node_modules` tiene miles de `node_modules` anidados).
pub fn escanear_proyectos(raiz: &Path, ahora: SystemTime) -> Vec<Candidato> {
    let mut v = Vec::new();
    let mut it = WalkDir::new(raiz).follow_links(false).into_iter();
    while let Some(Ok(e)) = it.next() {
        if !e.file_type().is_dir() {
            continue;
        }
        let nombre = e.file_name().to_string_lossy();
        let padre = e.path().parent().unwrap_or(raiz);
        let cat = match nombre.as_ref() {
            // Exigimos el "archivo hermano" que prueba que es un artefacto regenerable.
            "target" if padre.join("Cargo.toml").is_file() => Some(Categoria::BuildRust),
            "node_modules" if padre.join("package.json").is_file() => Some(Categoria::NodeModules),
            "__pycache__" | ".pytest_cache" | ".mypy_cache" => Some(Categoria::CachePython),
            ".git" => {
                it.skip_current_dir(); // nunca miramos dentro de repos git
                None
            }
            _ => None,
        };
        if let Some(c) = cat {
            v.push(candidato(e.path().to_path_buf(), c, ahora));
            it.skip_current_dir();
        }
    }
    v
}

/// Cachés de usuario conocidas que existan bajo `home`.
pub fn caches_usuario(home: &Path, ahora: SystemTime) -> Vec<Candidato> {
    const RUTAS: &[&str] = &[
        ".cache",                 // XDG (Linux)
        "Library/Caches",         // macOS
        ".cargo/registry/cache",  // .crate descargados (se re-descargan)
        ".npm/_cacache",
        ".cache/pip",
        "AppData/Local/Temp",     // Windows
    ];
    RUTAS
        .iter()
        .map(|r| home.join(r))
        // .cache/pip está dentro de .cache: evitamos contar dos veces.
        .filter(|p| p.is_dir() && !(p.ends_with("pip") && home.join(".cache").is_dir()))
        .map(|p| candidato(p, Categoria::CacheUsuario, ahora))
        .collect()
}

/// Archivos (no dirs) directamente en un directorio temporal, del usuario actual
/// si es posible saberlo, más viejos que `edad`.
pub fn temporales(dir: &Path, edad: Duration, ahora: SystemTime) -> Vec<Candidato> {
    let Ok(rd) = fs::read_dir(dir) else { return vec![] };
    rd.flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter(|e| es_mio(&e.path()))
        .map(|e| candidato(e.path(), Categoria::Temporal, ahora))
        .filter(|c| c.inactivo >= edad)
        .collect()
}

/// En Unix, solo archivos cuyo dueño es el usuario actual (no tocar los de otros).
#[cfg(unix)]
fn es_mio(p: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    // Comparamos con el dueño del directorio home actual del proceso: aproximación
    // sin libc. Si no se puede saber, por seguridad decimos que NO es mío.
    let yo = std::env::var_os("HOME").and_then(|h| fs::metadata(h).ok()).map(|m| m.uid());
    match (yo, fs::symlink_metadata(p)) {
        (Some(uid), Ok(md)) => md.uid() == uid,
        _ => false,
    }
}
#[cfg(not(unix))]
fn es_mio(_p: &Path) -> bool {
    true
}

/// Motivo por el que una ruta se rechaza.
#[derive(Debug, PartialEq, Eq)]
pub enum Rechazo {
    NoExiste,
    EsSymlink,
    FueraDeRaices,
    EsRaizOHome,
}

/// Valida que `ruta` sea segura de borrar dadas las raíces permitidas.
pub fn ruta_segura(ruta: &Path, raices: &[PathBuf], home: Option<&Path>) -> Result<PathBuf, Rechazo> {
    let md = fs::symlink_metadata(ruta).map_err(|_| Rechazo::NoExiste)?;
    if md.file_type().is_symlink() {
        return Err(Rechazo::EsSymlink);
    }
    let real = ruta.canonicalize().map_err(|_| Rechazo::NoExiste)?;
    // Nunca la raíz del FS ni el home.
    let home_real = home.and_then(|h| h.canonicalize().ok());
    if real.parent().is_none() || Some(&real) == home_real.as_ref() {
        return Err(Rechazo::EsRaizOHome);
    }
    let dentro = raices.iter().filter_map(|r| r.canonicalize().ok()).any(|r| real.starts_with(&r) && real != r);
    if !dentro {
        return Err(Rechazo::FueraDeRaices);
    }
    Ok(real)
}

/// Cómo eliminar.
#[derive(Debug, Clone)]
pub enum Modo {
    Borrar,
    /// Mover a una carpeta "papelera" (recuperable).
    Papelera(PathBuf),
}

/// Elimina los candidatos tras re-validarlos. Devuelve bytes liberados y errores.
pub fn limpiar(c: &[Candidato], raices: &[PathBuf], home: Option<&Path>, modo: &Modo) -> (u64, Vec<(PathBuf, String)>) {
    let mut liberados = 0;
    let mut errores = Vec::new();
    for (i, x) in c.iter().enumerate() {
        let real = match ruta_segura(&x.ruta, raices, home) {
            Ok(r) => r,
            Err(e) => {
                errores.push((x.ruta.clone(), format!("rechazada: {e:?}")));
                continue;
            }
        };
        let r = match modo {
            Modo::Borrar if real.is_dir() => fs::remove_dir_all(&real),
            Modo::Borrar => fs::remove_file(&real),
            Modo::Papelera(p) => mover_a_papelera(&real, p, i),
        };
        match r {
            Ok(()) => liberados += x.bytes,
            Err(e) => errores.push((x.ruta.clone(), e.to_string())),
        }
    }
    (liberados, errores)
}

/// Mueve con nombre único `NNN_nombre`. `rename` falla entre discos distintos:
/// en ese caso se informa el error (no copiamos gigas silenciosamente).
fn mover_a_papelera(real: &Path, papelera: &Path, i: usize) -> io::Result<()> {
    fs::create_dir_all(papelera)?;
    let nombre = real.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let ts = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
    fs::rename(real, papelera.join(format!("{ts}_{i:03}_{nombre}")))
}

/// Formatea bytes legibles.
pub fn humano(b: u64) -> String {
    let u = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut x = b as f64;
    let mut i = 0;
    while x >= 1024.0 && i < u.len() - 1 {
        x /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{b} B") } else { format!("{x:.1} {}", u[i]) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn arch(p: &Path, n: usize) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![b'x'; n]).unwrap();
    }

    #[test]
    fn detecta_artefactos_solo_con_archivo_hermano() {
        let t = tempdir().unwrap();
        let r = t.path();
        arch(&r.join("proy/Cargo.toml"), 1);
        arch(&r.join("proy/target/debug/bin"), 100);
        arch(&r.join("otro/target/importante.txt"), 5); // sin Cargo.toml: NO
        arch(&r.join("web/package.json"), 1);
        arch(&r.join("web/node_modules/a/node_modules/b.js"), 50);
        arch(&r.join("py/__pycache__/m.pyc"), 7);
        arch(&r.join(".git/x/Cargo.toml"), 1);
        let mut v = escanear_proyectos(r, SystemTime::now());
        v.sort_by_key(|c| c.categoria);
        let cats: Vec<_> = v.iter().map(|c| (c.categoria, c.bytes)).collect();
        assert_eq!(cats, vec![(Categoria::BuildRust, 100), (Categoria::NodeModules, 50), (Categoria::CachePython, 7)]);
    }

    #[test]
    fn caches_de_usuario_sin_doble_conteo() {
        let t = tempdir().unwrap();
        arch(&t.path().join(".cache/pip/x"), 10);
        arch(&t.path().join(".npm/_cacache/y"), 20);
        let v = caches_usuario(t.path(), SystemTime::now());
        assert_eq!(v.len(), 2);
        assert_eq!(v.iter().map(|c| c.bytes).sum::<u64>(), 30);
    }

    #[test]
    fn temporales_respetan_antiguedad() {
        let t = tempdir().unwrap();
        arch(&t.path().join("viejo.tmp"), 3);
        arch(&t.path().join("nuevo.tmp"), 3);
        let hace_10_dias = SystemTime::now() - Duration::from_secs(10 * 86400);
        fs::File::options().write(true).open(t.path().join("viejo.tmp")).unwrap().set_modified(hace_10_dias).unwrap();
        let v = temporales(t.path(), Duration::from_secs(7 * 86400), SystemTime::now());
        // En Unix además se filtra por dueño: tempdir es nuestro, así que aparece.
        assert_eq!(v.len(), 1);
        assert!(v[0].ruta.ends_with("viejo.tmp"));
    }

    #[test]
    fn validacion_de_rutas() {
        let t = tempdir().unwrap();
        let raiz = t.path().to_path_buf();
        arch(&raiz.join("a/b"), 1);
        let raices = vec![raiz.join("a")];
        assert!(ruta_segura(&raiz.join("a/b"), &raices, None).is_ok());
        assert_eq!(ruta_segura(&raiz.join("a"), &raices, None), Err(Rechazo::FueraDeRaices)); // la raíz misma no
        assert_eq!(ruta_segura(&raiz.join("a/../a/../x"), &raices, None), Err(Rechazo::NoExiste));
        arch(&raiz.join("fuera"), 1);
        assert_eq!(ruta_segura(&raiz.join("a/../fuera"), &raices, None), Err(Rechazo::FueraDeRaices));
        assert_eq!(ruta_segura(Path::new("/"), &[PathBuf::from("/")], None), Err(Rechazo::EsRaizOHome));
        assert_eq!(ruta_segura(&raiz.join("a"), &[raiz.clone()], Some(&raiz.join("a"))), Err(Rechazo::EsRaizOHome));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(raiz.join("fuera"), raiz.join("a/enlace")).unwrap();
            assert_eq!(ruta_segura(&raiz.join("a/enlace"), &raices, None), Err(Rechazo::EsSymlink));
        }
    }

    #[test]
    fn limpiar_borra_y_mueve_a_papelera() {
        let t = tempdir().unwrap();
        let r = t.path().join("ws");
        arch(&r.join("p/Cargo.toml"), 1);
        arch(&r.join("p/target/x"), 10);
        arch(&r.join("q/package.json"), 1);
        arch(&r.join("q/node_modules/y"), 20);
        let v = escanear_proyectos(&r, SystemTime::now());
        let raices = vec![r.clone()];
        let (lib, err) = limpiar(&v[..1], &raices, None, &Modo::Borrar);
        assert!(err.is_empty());
        assert!(lib > 0);
        let pap = t.path().join("papelera");
        let (_, err) = limpiar(&v[1..], &raices, None, &Modo::Papelera(pap.clone()));
        assert!(err.is_empty());
        assert!(!r.join("p/target").exists() && !r.join("q/node_modules").exists());
        assert_eq!(fs::read_dir(&pap).unwrap().count(), 1);
        // Los archivos de proyecto siguen ahí.
        assert!(r.join("p/Cargo.toml").exists());
    }

    #[test]
    fn formato_humano() {
        assert_eq!(humano(512), "512 B");
        assert_eq!(humano(1536), "1.5 KiB");
        assert_eq!(humano(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }
}
