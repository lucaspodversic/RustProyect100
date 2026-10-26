//! # Balanceador L4 TCP
//!
//! Round-robin + opción least-connections. Health-check periódico
//! (intento de connect). Backends muertos se saltean.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

#[derive(Debug)]
pub struct Backend {
    pub addr: String,
    pub alive: bool,
    pub active: AtomicUsize,
}

impl Backend {
    pub fn new(addr: impl Into<String>) -> Self {
        Self { addr: addr.into(), alive: true, active: AtomicUsize::new(0) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy { RoundRobin, LeastConn }

pub struct Balancer {
    pub backends: Mutex<Vec<Backend>>,
    pub strategy: Strategy,
    rr: AtomicUsize,
}

impl Balancer {
    pub fn new(addrs: Vec<String>, strategy: Strategy) -> Arc<Self> {
        Arc::new(Self {
            backends: Mutex::new(addrs.into_iter().map(Backend::new).collect()),
            strategy,
            rr: AtomicUsize::new(0),
        })
    }

    pub async fn pick(self: &Arc<Self>) -> Option<String> {
        let g = self.backends.lock().await;
        let alive: Vec<_> = g.iter().enumerate().filter(|(_, b)| b.alive).collect();
        if alive.is_empty() { return None; }
        let idx = match self.strategy {
            Strategy::RoundRobin => {
                let i = self.rr.fetch_add(1, Ordering::Relaxed) % alive.len();
                alive[i].0
            }
            Strategy::LeastConn => {
                alive.iter().min_by_key(|(_, b)| b.active.load(Ordering::Relaxed)).map(|(i, _)| *i).unwrap()
            }
        };
        g[idx].active.fetch_add(1, Ordering::Relaxed);
        Some(g[idx].addr.clone())
    }

    pub async fn release(self: &Arc<Self>, addr: &str) {
        let g = self.backends.lock().await;
        if let Some(b) = g.iter().find(|b| b.addr == addr) {
            b.active.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub async fn health_loop(self: Arc<Self>, every: Duration) {
        loop {
            let addrs: Vec<String> = {
                let g = self.backends.lock().await;
                g.iter().map(|b| b.addr.clone()).collect()
            };
            for (i, addr) in addrs.into_iter().enumerate() {
                let ok = TcpStream::connect(&addr).await.is_ok();
                self.backends.lock().await[i].alive = ok;
            }
            sleep(every).await;
        }
    }

    pub async fn serve(self: Arc<Self>, listener: TcpListener) -> std::io::Result<()> {
        loop {
            let (inbound, _) = listener.accept().await?;
            let bal = self.clone();
            tokio::spawn(async move {
                if let Some(addr) = bal.pick().await {
                    if let Ok(outbound) = TcpStream::connect(&addr).await {
                        let (mut ri, mut wi) = inbound.into_split();
                        let (mut ro, mut wo) = outbound.into_split();
                        let c1 = tokio::spawn(async move {
                            let _ = tokio::io::copy(&mut ri, &mut wo).await;
                            let _ = wo.shutdown().await;
                        });
                        let c2 = tokio::spawn(async move {
                            let _ = tokio::io::copy(&mut ro, &mut wi).await;
                            let _ = wi.shutdown().await;
                        });
                        let _ = c1.await;
                        let _ = c2.await;
                    }
                    bal.release(&addr).await;
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn echo() -> String {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let a = l.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = l.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut buf = [0u8; 64];
                    if let Ok(n) = s.read(&mut buf).await { let _ = s.write_all(&buf[..n]).await; }
                });
            }
        });
        a
    }

    #[tokio::test]
    async fn round_robin() {
        let b1 = echo().await;
        let b2 = echo().await;
        let bal = Balancer::new(vec![b1.clone(), b2.clone()], Strategy::RoundRobin);
        assert_eq!(bal.pick().await.unwrap(), b1);
        assert_eq!(bal.pick().await.unwrap(), b2);
        // least-conn
        let bal2 = Balancer::new(vec![b1.clone(), b2.clone()], Strategy::LeastConn);
        let a = bal2.pick().await.unwrap();
        let b = bal2.pick().await.unwrap();
        assert_ne!(a, b);
        bal2.release(&a).await;
        // proxy e2e
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let bal3 = Balancer::new(vec![b1, b2], Strategy::RoundRobin);
        tokio::spawn(bal3.serve(l));
        let mut c = TcpStream::connect(&addr).await.unwrap();
        c.write_all(b"hey").await.unwrap();
        let mut buf = [0u8; 3];
        c.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hey");
    }
}
