//! # util-linux (mini)
//!
//! Reimplementación educativa de tres utilidades:
//! - `tail`: últimas N líneas (o follow básico leyendo lo nuevo).
//! - `mountpoint`: ¿es `path` un punto de montaje? (compara device ids).
//! - `wc`: cuenta líneas/palabras/bytes.

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Últimas `n` líneas de un archivo.
pub fn tail_lines(path: &Path, n: usize) -> io::Result<Vec<String>> {
    let f = File::open(path)?;
    let lines: Vec<String> = BufReader::new(f).lines().collect::<Result<_, _>>()?;
    let start = lines.len().saturating_sub(n);
    Ok(lines[start..].to_vec())
}

/// Lee bytes nuevos desde `offset` (para un "follow" simple).
pub fn tail_follow_once(path: &Path, offset: &mut u64) -> io::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    let meta = f.metadata()?;
    if meta.len() < *offset {
        *offset = 0; // el archivo fue truncado
    }
    f.seek(SeekFrom::Start(*offset))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    *offset += buf.len() as u64;
    Ok(buf)
}

/// ¿`path` es un mountpoint? Compara el `dev` del path con el de su padre.
/// (En Linux real `mountpoint(1)` también mira `/proc/self/mountinfo`.)
pub fn is_mountpoint(path: &Path) -> io::Result<bool> {
    let meta = fs::metadata(path)?;
    if !meta.is_dir() {
        return Ok(false);
    }
    let parent = path.parent().unwrap_or(Path::new("/"));
    // Si path es "/", es mountpoint por definición.
    if path == Path::new("/") || path.components().count() == 0 {
        return Ok(true);
    }
    let parent_meta = fs::metadata(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(meta.dev() != parent_meta.dev() || meta.ino() == parent_meta.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = parent_meta;
        Ok(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wc {
    pub lines: usize,
    pub words: usize,
    pub bytes: usize,
}

pub fn wc(data: &[u8]) -> Wc {
    let bytes = data.len();
    let lines = data.iter().filter(|&&b| b == b'\n').count();
    let words = std::str::from_utf8(data)
        .map(|s| s.split_whitespace().count())
        .unwrap_or(0);
    Wc { lines, words, bytes }
}

pub fn wc_file(path: &Path) -> io::Result<Wc> {
    Ok(wc(&fs::read(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn tail_y_follow() {
        let dir = std::env::temp_dir().join(format!("util-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.txt");
        fs::write(&p, b"1\n2\n3\n4\n5\n").unwrap();
        assert_eq!(tail_lines(&p, 2).unwrap(), vec!["4", "5"]);
        let mut off = fs::metadata(&p).unwrap().len();
        let mut f = fs::OpenOptions::new().append(true).open(&p).unwrap();
        write!(f, "6\n").unwrap();
        assert_eq!(tail_follow_once(&p, &mut off).unwrap(), b"6\n");
        assert!(tail_follow_once(&p, &mut off).unwrap().is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn wc_counts() {
        assert_eq!(wc(b"hola mundo\nchau\n"), Wc { lines: 2, words: 3, bytes: 16 });
    }

    #[test]
    fn mountpoint_root() {
        assert!(is_mountpoint(Path::new("/")).unwrap());
        // Un directorio normal del tmp no debería ser mountpoint.
        let dir = std::env::temp_dir().join(format!("utilmp-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(!is_mountpoint(&dir).unwrap());
        fs::remove_dir_all(dir).unwrap();
    }
}
