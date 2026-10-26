//! # Benchmark de red
//!
//! Mide **latencia** (RTT de un ping TCP) y **throughput** (bytes/s)
//! contra un endpoint. Los tests levantan un eco/sink en 127.0.0.1.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct LatencyReport {
    pub samples: Vec<Duration>,
    pub min: Duration,
    pub max: Duration,
    pub mean: Duration,
    pub p50: Duration,
    pub p99: Duration,
}

impl LatencyReport {
    pub fn from_samples(mut samples: Vec<Duration>) -> Self {
        samples.sort();
        let n = samples.len().max(1);
        let sum: Duration = samples.iter().copied().sum();
        let pct = |p: f64| samples[((p * (n - 1) as f64).round() as usize).min(n - 1)];
        Self {
            min: samples[0],
            max: *samples.last().unwrap(),
            mean: sum / n as u32,
            p50: pct(0.50),
            p99: pct(0.99),
            samples,
        }
    }
}

/// Mide RTT: escribe 1 byte y espera 1 byte de eco, `n` veces.
pub fn measure_latency(addr: &str, n: usize) -> std::io::Result<LatencyReport> {
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let mut s = TcpStream::connect(addr)?;
        s.set_nodelay(true)?;
        let t0 = Instant::now();
        s.write_all(&[0x42])?;
        let mut b = [0u8; 1];
        s.read_exact(&mut b)?;
        samples.push(t0.elapsed());
    }
    Ok(LatencyReport::from_samples(samples))
}

#[derive(Debug, Clone)]
pub struct ThroughputReport {
    pub bytes: u64,
    pub elapsed: Duration,
    pub mbps: f64,
}

/// Envía `total` bytes en chunks de `chunk` y mide el tiempo.
pub fn measure_throughput(addr: &str, total: usize, chunk: usize) -> std::io::Result<ThroughputReport> {
    let mut s = TcpStream::connect(addr)?;
    s.set_nodelay(true)?;
    let buf = vec![0u8; chunk];
    let t0 = Instant::now();
    let mut sent = 0;
    while sent < total {
        let n = chunk.min(total - sent);
        s.write_all(&buf[..n])?;
        sent += n;
    }
    s.flush()?;
    // Señal de fin: cerrar escritura y leer ACK de 1 byte.
    let _ = s.shutdown(std::net::Shutdown::Write);
    let mut ack = [0u8; 1];
    let _ = s.read(&mut ack);
    let elapsed = t0.elapsed();
    let mbps = (sent as f64 * 8.0) / elapsed.as_secs_f64() / 1_000_000.0;
    Ok(ThroughputReport { bytes: sent as u64, elapsed, mbps })
}

/// Eco: lee 1 byte y lo reenvía (para latencia).
pub fn spawn_echo() -> std::io::Result<String> {
    let l = TcpListener::bind("127.0.0.1:0")?;
    let addr = l.local_addr()?.to_string();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut s = s;
            let mut b = [0u8; 1];
            if s.read_exact(&mut b).is_ok() {
                let _ = s.write_all(&b);
            }
        }
    });
    Ok(addr)
}

/// Sink: consume todo y responde 1 byte al EOF.
pub fn spawn_sink() -> std::io::Result<String> {
    let l = TcpListener::bind("127.0.0.1:0")?;
    let addr = l.local_addr()?.to_string();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut s = s;
            let mut buf = [0u8; 8192];
            loop {
                match s.read(&mut buf) {
                    Ok(0) => { let _ = s.write_all(&[1]); break; }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }
    });
    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latencia_local() {
        let addr = spawn_echo().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        let r = measure_latency(&addr, 20).unwrap();
        assert_eq!(r.samples.len(), 20);
        assert!(r.min <= r.p50 && r.p50 <= r.p99 && r.p99 <= r.max);
        assert!(r.mean < Duration::from_millis(50), "RTT local debería ser bajo");
    }
    #[test]
    fn throughput_local() {
        let addr = spawn_sink().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        let r = measure_throughput(&addr, 2 * 1024 * 1024, 32 * 1024).unwrap();
        assert_eq!(r.bytes, 2 * 1024 * 1024);
        assert!(r.mbps > 100.0, "throughput local: {} Mbps", r.mbps);
    }
}
