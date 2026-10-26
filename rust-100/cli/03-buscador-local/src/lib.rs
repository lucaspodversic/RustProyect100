//! # Buscador local — lógica
//!
//! Un "ripgrep" simplificado en dos etapas:
//! 1. **Recolección** ([`recolectar`]): recorre el árbol con `walkdir`, saltea
//!    directorios ignorados (`.git`, `target`, `node_modules`, ocultos) y filtra
//!    por nombre (regex sobre el nombre de archivo) y por extensión.
//! 2. **Búsqueda de contenido** ([`buscar_contenido`]): en paralelo con `rayon`
//!    lee cada archivo, descarta binarios (heurística de byte NUL) y devuelve
//!    las líneas que coinciden con la regex, con número de línea.

use rayon::prelude::*;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

/// Directorios que nunca vale la pena recorrer (como hace ripgrep por defecto).
pub const IGNORADOS: &[&str] = &[".git", "target", "node_modules", ".hg", ".svn", "__pycache__"];

/// Opciones de búsqueda.
#[derive(Debug, Clone, Default)]
pub struct Opciones {
    /// Regex sobre el NOMBRE de archivo (no la ruta completa).
    pub nombre: Option<Regex>,
    /// Extensiones permitidas (sin punto). Vacío = todas.
    pub extensiones: Vec<String>,
    /// Incluir archivos y carpetas ocultos.
    pub ocultos: bool,
    /// Profundidad máxima de recursión.
    pub max_profundidad: Option<usize>,
    /// Tamaño máximo de archivo a inspeccionar (bytes); evita leer archivos gigantes.
    pub max_bytes: u64,
}

/// Una coincidencia de contenido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coincidencia {
    pub ruta: PathBuf,
    /// Número de línea, empezando en 1.
    pub linea: usize,
    pub texto: String,
}

/// ¿Es un directorio/archivo que debemos saltear?
fn es_ignorado(e: &DirEntry, ocultos: bool) -> bool {
    // La raíz (profundidad 0) nunca se ignora aunque se llame ".".
    if e.depth() == 0 {
        return false;
    }
    let nombre = e.file_name().to_string_lossy();
    if e.file_type().is_dir() && IGNORADOS.contains(&nombre.as_ref()) {
        return true;
    }
    !ocultos && nombre.starts_with('.')
}

/// Recorre `raiz` y devuelve los archivos que pasan los filtros de nombre/extensión.
pub fn recolectar(raiz: &Path, o: &Opciones) -> Vec<PathBuf> {
    let mut w = WalkDir::new(raiz).follow_links(false);
    if let Some(p) = o.max_profundidad {
        w = w.max_depth(p);
    }
    let mut v: Vec<PathBuf> = w
        .into_iter()
        // `filter_entry` poda subárboles enteros: no entra a .git, target, etc.
        .filter_entry(|e| !es_ignorado(e, o.ocultos))
        .filter_map(Result::ok) // errores de permisos: se ignoran silenciosamente
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let nombre = e.file_name().to_string_lossy();
            o.nombre.as_ref().is_none_or(|re| re.is_match(&nombre))
        })
        .filter(|e| {
            o.extensiones.is_empty()
                || e.path()
                    .extension()
                    .map(|x| o.extensiones.iter().any(|w| w.eq_ignore_ascii_case(&x.to_string_lossy())))
                    .unwrap_or(false)
        })
        .map(|e| e.into_path())
        .collect();
    v.sort(); // salida determinista
    v
}

/// Heurística de binario: si hay un byte 0 en los primeros 8 KiB, es binario.
pub fn parece_binario(bytes: &[u8]) -> bool {
    bytes.iter().take(8192).any(|&b| b == 0)
}

/// Busca `re` dentro de un archivo. Devuelve vacío si es binario, grande o ilegible.
pub fn buscar_en_archivo(ruta: &Path, re: &Regex, max_bytes: u64) -> Vec<Coincidencia> {
    let Ok(meta) = fs::metadata(ruta) else { return vec![] };
    if max_bytes > 0 && meta.len() > max_bytes {
        return vec![];
    }
    let Ok(bytes) = fs::read(ruta) else { return vec![] };
    if parece_binario(&bytes) {
        return vec![];
    }
    // `from_utf8_lossy` tolera archivos Latin-1 u otros con bytes inválidos.
    let texto = String::from_utf8_lossy(&bytes);
    texto
        .lines()
        .enumerate()
        .filter(|(_, l)| re.is_match(l))
        .map(|(i, l)| Coincidencia { ruta: ruta.to_path_buf(), linea: i + 1, texto: l.to_string() })
        .collect()
}

/// Búsqueda de contenido en paralelo sobre la lista de archivos.
/// Rayon reparte los archivos entre hilos (work-stealing); luego ordenamos.
pub fn buscar_contenido(archivos: &[PathBuf], re: &Regex, max_bytes: u64) -> Vec<Coincidencia> {
    let mut v: Vec<Coincidencia> = archivos.par_iter().flat_map(|p| buscar_en_archivo(p, re, max_bytes)).collect();
    v.sort_by(|a, b| a.ruta.cmp(&b.ruta).then(a.linea.cmp(&b.linea)));
    v
}

/// Resalta las coincidencias con códigos ANSI (rojo negrita) para la terminal.
pub fn resaltar(linea: &str, re: &Regex) -> String {
    re.replace_all(linea, "\x1b[1;31m$0\x1b[0m").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Arma un árbol de prueba:
    /// raiz/a.rs, raiz/b.txt, raiz/sub/c.rs, raiz/.oculto.rs, raiz/target/x.rs, raiz/bin.dat
    fn arbol() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        fs::write(r.join("a.rs"), "fn main() {}\n// TODO: arreglar\n").unwrap();
        fs::write(r.join("b.txt"), "hola mundo\nTODO otra cosa\n").unwrap();
        fs::create_dir_all(r.join("sub")).unwrap();
        fs::write(r.join("sub/c.rs"), "nada\n").unwrap();
        fs::write(r.join(".oculto.rs"), "TODO\n").unwrap();
        fs::create_dir_all(r.join("target")).unwrap();
        fs::write(r.join("target/x.rs"), "TODO\n").unwrap();
        fs::write(r.join("bin.dat"), [b'T', b'O', 0, b'D', b'O']).unwrap();
        d
    }

    #[test]
    fn recolecta_ignorando_target_y_ocultos() {
        let d = arbol();
        let v = recolectar(d.path(), &Opciones::default());
        let nombres: Vec<_> = v.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(nombres, vec!["a.rs", "b.txt", "bin.dat", "c.rs"]);
    }

    #[test]
    fn filtra_por_extension_y_nombre() {
        let d = arbol();
        let o = Opciones { extensiones: vec!["rs".into()], ..Default::default() };
        assert_eq!(recolectar(d.path(), &o).len(), 2);
        let o = Opciones { nombre: Some(Regex::new("^b").unwrap()), ..Default::default() };
        assert_eq!(recolectar(d.path(), &o).len(), 2); // b.txt y bin.dat
    }

    #[test]
    fn ocultos_y_profundidad() {
        let d = arbol();
        let o = Opciones { ocultos: true, ..Default::default() };
        assert_eq!(recolectar(d.path(), &o).len(), 5);
        let o = Opciones { max_profundidad: Some(1), ..Default::default() };
        assert_eq!(recolectar(d.path(), &o).len(), 3);
    }

    #[test]
    fn busca_contenido_y_saltea_binarios() {
        let d = arbol();
        let files = recolectar(d.path(), &Opciones::default());
        let re = Regex::new("TODO").unwrap();
        let m = buscar_contenido(&files, &re, 0);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].linea, 2); // a.rs línea 2
        assert!(m[1].ruta.ends_with("b.txt"));
    }

    #[test]
    fn respeta_max_bytes() {
        let d = arbol();
        let re = Regex::new("hola").unwrap();
        assert!(buscar_en_archivo(&d.path().join("b.txt"), &re, 3).is_empty());
        assert_eq!(buscar_en_archivo(&d.path().join("b.txt"), &re, 0).len(), 1);
    }

    #[test]
    fn resaltado_ansi() {
        let re = Regex::new("o").unwrap();
        assert_eq!(resaltar("so", &re), "s\x1b[1;31mo\x1b[0m");
    }
}
