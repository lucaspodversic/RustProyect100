//! # Transferencia de archivos TCP con progreso
//!
//! Protocolo: `[u64 BE size][nombre utf-8 length-prefixed u16][bytes...]`.
//! El receptor reporta progreso vía callback.

use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

pub fn send_file(stream: &mut TcpStream, path: &Path) -> std::io::Result<u64> {
    let mut f = File::open(path)?;
    let size = f.metadata()?.len();
    let name = path.file_name().unwrap().to_string_lossy();
    let name_b = name.as_bytes();
    stream.write_all(&size.to_be_bytes())?;
    stream.write_all(&(name_b.len() as u16).to_be_bytes())?;
    stream.write_all(name_b)?;
    let mut buf = [0u8; 8192];
    let mut sent = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 { break; }
        stream.write_all(&buf[..n])?;
        sent += n as u64;
    }
    Ok(sent)
}

pub fn recv_file(
    stream: &mut TcpStream,
    dest_dir: &Path,
    mut on_progress: impl FnMut(u64, u64),
) -> std::io::Result<(String, u64)> {
    let mut sizeb = [0u8; 8];
    stream.read_exact(&mut sizeb)?;
    let size = u64::from_be_bytes(sizeb);
    let mut nlenb = [0u8; 2];
    stream.read_exact(&mut nlenb)?;
    let nlen = u16::from_be_bytes(nlenb) as usize;
    let mut nameb = vec![0u8; nlen];
    stream.read_exact(&mut nameb)?;
    let name = String::from_utf8(nameb).unwrap_or_else(|_| "file.bin".into());
    // Evitar path traversal.
    let safe = name.replace(['/', '\\', ':'], "_");
    let dest = dest_dir.join(&safe);
    let mut out = File::create(&dest)?;
    let mut buf = [0u8; 8192];
    let mut got = 0u64;
    while got < size {
        let want = ((size - got) as usize).min(buf.len());
        stream.read_exact(&mut buf[..want])?;
        out.write_all(&buf[..want])?;
        got += want as u64;
        on_progress(got, size);
    }
    Ok((safe, got))
}

pub fn serve_once(listener: &TcpListener, path: &Path) -> std::io::Result<u64> {
    let (mut s, _) = listener.accept()?;
    send_file(&mut s, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    #[test]
    fn transfer_con_progreso() {
        let dir = std::env::temp_dir().join(format!("ft-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("hola.txt");
        let data = vec![b'x'; 50_000];
        std::fs::write(&src, &data).unwrap();
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let src2 = src.clone();
        std::thread::spawn(move || { let _ = serve_once(&l, &src2); });
        let mut s = TcpStream::connect(addr).unwrap();
        let dest = dir.join("out");
        std::fs::create_dir_all(&dest).unwrap();
        let progress = Arc::new(Mutex::new(Vec::new()));
        let p2 = progress.clone();
        let (name, n) = recv_file(&mut s, &dest, |got, total| {
            p2.lock().unwrap().push((got, total));
        }).unwrap();
        assert_eq!(name, "hola.txt");
        assert_eq!(n, data.len() as u64);
        assert_eq!(std::fs::read(dest.join("hola.txt")).unwrap(), data);
        let p = progress.lock().unwrap();
        assert!(!p.is_empty());
        assert_eq!(p.last().unwrap().0, data.len() as u64);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
