//! # Descubrimiento tipo mDNS simplificado
//!
//! Anuncios UDP: `ANNOUNCE <nombre> <host> <puerto>` y consultas
//! `QUERY <nombre>`. Respuestas `REPLY <nombre> <host> <puerto>`.
//! No es mDNS/DNS-SD real (multicast 224.0.0.251:5353); es el mismo
//! patrón sobre unicast/broadcast local para poder testear sin root.

use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    pub name: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Default)]
pub struct Registry {
    pub local: HashMap<String, Service>,
    pub discovered: HashMap<String, (Service, Instant)>,
}

impl Registry {
    pub fn announce(&mut self, svc: Service) {
        self.local.insert(svc.name.clone(), svc);
    }

    pub fn handle(&mut self, msg: &str) -> Option<String> {
        let p: Vec<&str> = msg.split_whitespace().collect();
        match p.as_slice() {
            ["QUERY", name] => self.local.get(*name).map(|s| {
                format!("REPLY {} {} {}", s.name, s.host, s.port)
            }),
            ["ANNOUNCE", name, host, port] => {
                if let Ok(port) = port.parse() {
                    self.discovered.insert(
                        (*name).into(),
                        (Service { name: (*name).into(), host: (*host).into(), port }, Instant::now()),
                    );
                }
                None
            }
            ["REPLY", name, host, port] => {
                if let Ok(port) = port.parse() {
                    self.discovered.insert(
                        (*name).into(),
                        (Service { name: (*name).into(), host: (*host).into(), port }, Instant::now()),
                    );
                }
                None
            }
            _ => None,
        }
    }

    pub fn lookup(&self, name: &str, max_age: Duration) -> Option<Service> {
        self.discovered.get(name).and_then(|(s, t)| {
            if t.elapsed() <= max_age { Some(s.clone()) } else { None }
        })
    }
}

pub fn serve(sock: UdpSocket, reg: Arc<Mutex<Registry>>) {
    let mut buf = [0u8; 512];
    loop {
        if let Ok((n, src)) = sock.recv_from(&mut buf) {
            let msg = String::from_utf8_lossy(&buf[..n]).to_string();
            let reply = reg.lock().unwrap().handle(&msg);
            if let Some(r) = reply {
                let _ = sock.send_to(r.as_bytes(), src);
            }
        }
    }
}

pub fn query(sock: &UdpSocket, server: &str, name: &str) -> std::io::Result<Option<Service>> {
    sock.send_to(format!("QUERY {name}").as_bytes(), server)?;
    sock.set_read_timeout(Some(Duration::from_millis(300)))?;
    let mut buf = [0u8; 512];
    match sock.recv_from(&mut buf) {
        Ok((n, _)) => {
            let msg = String::from_utf8_lossy(&buf[..n]);
            let mut reg = Registry::default();
            reg.handle(&msg);
            Ok(reg.lookup(name, Duration::from_secs(60)))
        }
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn anuncio_y_query() {
        let reg = Arc::new(Mutex::new(Registry::default()));
        reg.lock().unwrap().announce(Service {
            name: "_http._tcp.local".into(),
            host: "127.0.0.1".into(),
            port: 8080,
        });
        let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = sock.local_addr().unwrap().to_string();
        let reg2 = reg.clone();
        std::thread::spawn(move || serve(sock, reg2));
        let cli = UdpSocket::bind("127.0.0.1:0").unwrap();
        let svc = query(&cli, &addr, "_http._tcp.local").unwrap().unwrap();
        assert_eq!(svc.port, 8080);
        assert_eq!(svc.host, "127.0.0.1");
    }
}
