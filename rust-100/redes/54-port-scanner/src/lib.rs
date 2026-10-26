//! # Escáner de puertos concurrente
//!
//! Intenta `TcpStream::connect_timeout` a un rango de puertos con un
//! pool de workers. Tests levantan listeners locales.

use std::net::{SocketAddr, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status { Open, Closed, Filtered }

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub port: u16,
    pub status: Status,
}

/// Escanea `ports` en `host` con `workers` hilos y `timeout` por intento.
pub fn scan(host: &str, ports: &[u16], workers: usize, timeout: Duration) -> Vec<ScanResult> {
    let q = Arc::new(Mutex::new(ports.to_vec()));
    let (tx, rx) = mpsc::channel();
    let mut hs = Vec::new();
    for _ in 0..workers.max(1) {
        let q = q.clone();
        let tx = tx.clone();
        let host = host.to_string();
        hs.push(thread::spawn(move || {
            loop {
                let port = { q.lock().unwrap().pop() };
                let Some(port) = port else { break };
                let addr = format!("{host}:{port}");
                let status = match addr.parse::<SocketAddr>() {
                    Ok(a) => match TcpStream::connect_timeout(&a, timeout) {
                        Ok(_) => Status::Open,
                        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => Status::Closed,
                        Err(_) => Status::Filtered,
                    },
                    Err(_) => Status::Filtered,
                };
                let _ = tx.send(ScanResult { port, status });
            }
        }));
    }
    drop(tx);
    for h in hs { let _ = h.join(); }
    let mut out: Vec<_> = rx.into_iter().collect();
    out.sort_by_key(|r| r.port);
    out
}

pub fn open_ports(host: &str, ports: &[u16], workers: usize, timeout: Duration) -> Vec<u16> {
    scan(host, ports, workers, timeout).into_iter().filter(|r| r.status == Status::Open).map(|r| r.port).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    #[test]
    fn detecta_abiertos_y_cerrados() {
        let l1 = TcpListener::bind("127.0.0.1:0").unwrap();
        let l2 = TcpListener::bind("127.0.0.1:0").unwrap();
        let p1 = l1.local_addr().unwrap().port();
        let p2 = l2.local_addr().unwrap().port();
        std::thread::spawn(move || for _ in l1.incoming() {});
        std::thread::spawn(move || for _ in l2.incoming() {});
        let closed = { let l = TcpListener::bind("127.0.0.1:0").unwrap(); let p = l.local_addr().unwrap().port(); drop(l); p };
        let ports = [p1, p2, closed];
        let r = scan("127.0.0.1", &ports, 4, Duration::from_millis(200));
        assert_eq!(r.iter().find(|x| x.port == p1).unwrap().status, Status::Open);
        assert_eq!(r.iter().find(|x| x.port == p2).unwrap().status, Status::Open);
        assert_eq!(r.iter().find(|x| x.port == closed).unwrap().status, Status::Closed);
        let open = open_ports("127.0.0.1", &ports, 4, Duration::from_millis(200));
        assert!(open.contains(&p1) && open.contains(&p2));
    }
}
