//! # Proxy inverso TCP asíncrono
//!
//! Balanceo **round-robin** entre backends. Cada conexión entrante se
//! empareja con un backend y se copian bytes en ambos sentidos con tokio.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone)]
pub struct Balancer {
    backends: Arc<Vec<String>>,
    idx: Arc<AtomicUsize>,
}

impl Balancer {
    pub fn new(backends: Vec<String>) -> Self {
        assert!(!backends.is_empty());
        Self { backends: Arc::new(backends), idx: Arc::new(AtomicUsize::new(0)) }
    }

    pub fn next(&self) -> &str {
        let i = self.idx.fetch_add(1, Ordering::Relaxed);
        &self.backends[i % self.backends.len()]
    }

    pub async fn serve(self, listener: TcpListener) -> std::io::Result<()> {
        loop {
            let (inbound, _) = listener.accept().await?;
            let backend = self.next().to_string();
            tokio::spawn(async move {
                let _ = proxy(inbound, &backend).await;
            });
        }
    }
}

async fn proxy(mut inbound: TcpStream, backend: &str) -> std::io::Result<()> {
    let mut outbound = TcpStream::connect(backend).await?;
    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();
    let c1 = async {
        let n = tokio::io::copy(&mut ri, &mut wo).await?;
        wo.shutdown().await?;
        Ok::<_, std::io::Error>(n)
    };
    let c2 = async {
        let n = tokio::io::copy(&mut ro, &mut wi).await?;
        wi.shutdown().await?;
        Ok::<_, std::io::Error>(n)
    };
    tokio::try_join!(c1, c2)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn echo_server() -> String {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = l.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 256];
                    if let Ok(n) = s.read(&mut buf).await {
                        let _ = s.write_all(&buf[..n]).await;
                    }
                });
            }
        });
        addr
    }

    #[tokio::test]
    async fn round_robin_y_proxy() {
        let b1 = echo_server().await;
        let b2 = echo_server().await;
        let bal = Balancer::new(vec![b1.clone(), b2.clone()]);
        assert_eq!(bal.next(), &b1);
        assert_eq!(bal.next(), &b2);
        assert_eq!(bal.next(), &b1);
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let bal2 = Balancer::new(vec![b1, b2]);
        tokio::spawn(async move { bal2.serve(l).await });
        let mut c = TcpStream::connect(&addr).await.unwrap();
        c.write_all(b"hola").await.unwrap();
        let mut buf = [0u8; 4];
        c.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hola");
    }
}
