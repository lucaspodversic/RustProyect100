//! # Proxy SOCKS5 simple
//!
//! Soporta: greeting sin auth (método 0), CONNECT a IPv4/dominio.
//! Tras el handshake, relay bidireccional de bytes.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

fn relay(mut a: TcpStream, mut b: TcpStream) {
    let mut a2 = a.try_clone().unwrap();
    let mut b2 = b.try_clone().unwrap();
    let t = std::thread::spawn(move || { let _ = std::io::copy(&mut a2, &mut b2); });
    let _ = std::io::copy(&mut b, &mut a);
    let _ = t.join();
}

pub fn handle(mut client: TcpStream) -> std::io::Result<()> {
    // greeting: VER NMETHODS METHODS...
    let mut buf = [0u8; 2];
    client.read_exact(&mut buf)?;
    if buf[0] != 5 { return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "no socks5")); }
    let mut methods = vec![0u8; buf[1] as usize];
    client.read_exact(&mut methods)?;
    if !methods.contains(&0) {
        client.write_all(&[5, 0xFF])?;
        return Ok(());
    }
    client.write_all(&[5, 0])?; // no auth

    // request: VER CMD RSV ATYP ...
    let mut hdr = [0u8; 4];
    client.read_exact(&mut hdr)?;
    if hdr[0] != 5 || hdr[1] != 1 {
        // sólo CONNECT
        client.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0])?;
        return Ok(());
    }
    let host = match hdr[3] {
        1 => { // IPv4
            let mut ip = [0u8; 4];
            client.read_exact(&mut ip)?;
            format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
        }
        3 => { // domain
            let mut l = [0u8; 1];
            client.read_exact(&mut l)?;
            let mut name = vec![0u8; l[0] as usize];
            client.read_exact(&mut name)?;
            String::from_utf8(name).unwrap_or_default()
        }
        _ => {
            client.write_all(&[5, 8, 0, 1, 0, 0, 0, 0, 0, 0])?;
            return Ok(());
        }
    };
    let mut portb = [0u8; 2];
    client.read_exact(&mut portb)?;
    let port = u16::from_be_bytes(portb);
    let target = format!("{host}:{port}");
    match TcpStream::connect(&target) {
        Ok(remote) => {
            // éxito: VER REP RSV ATYP BND.ADDR BND.PORT
            client.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0])?;
            relay(client, remote);
        }
        Err(_) => {
            client.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0])?; // connection refused
        }
    }
    Ok(())
}

pub fn serve(listener: TcpListener) {
    for s in listener.incoming().flatten() {
        std::thread::spawn(move || { let _ = handle(s); });
    }
}

/// Cliente SOCKS5 mínimo: CONNECT a host:port vía proxy.
pub fn connect_via(proxy: &str, host: &str, port: u16) -> std::io::Result<TcpStream> {
    let mut s = TcpStream::connect(proxy)?;
    s.write_all(&[5, 1, 0])?; // VER NMETHODS NOAUTH
    let mut resp = [0u8; 2];
    s.read_exact(&mut resp)?;
    if resp != [5, 0] { return Err(std::io::Error::new(std::io::ErrorKind::Other, "auth")); }
    let mut req = vec![5, 1, 0, 3, host.len() as u8];
    req.extend(host.as_bytes());
    req.extend_from_slice(&port.to_be_bytes());
    s.write_all(&req)?;
    let mut reply = [0u8; 10];
    s.read_exact(&mut reply)?; // asumimos ATYP=IPv4 en la respuesta
    if reply[1] != 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("REP {}", reply[1])));
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proxy_echo() {
        // backend eco
        let backend = TcpListener::bind("127.0.0.1:0").unwrap();
        let bport = backend.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for s in backend.incoming().flatten() {
                let mut s = s;
                let mut buf = [0u8; 64];
                if let Ok(n) = s.read(&mut buf) { let _ = s.write_all(&buf[..n]); }
            }
        });
        let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
        let paddr = proxy.local_addr().unwrap().to_string();
        std::thread::spawn(move || serve(proxy));
        std::thread::sleep(std::time::Duration::from_millis(30));
        let mut s = connect_via(&paddr, "127.0.0.1", bport).unwrap();
        s.write_all(b"hola").unwrap();
        let mut buf = [0u8; 4];
        s.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"hola");
    }
}
