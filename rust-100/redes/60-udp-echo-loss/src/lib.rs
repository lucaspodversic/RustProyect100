//! # Eco UDP con manejo de pérdida
//!
//! Cada datagrama: `[id u32][payload...]`. El cliente reenvía hasta
//! recibir ACK (eco del mismo id) o agotar reintentos. El servidor puede
//! simular pérdida descartando un porcentaje.

use std::net::UdpSocket;
use std::time::Duration;

pub fn send_reliable(
    sock: &UdpSocket,
    addr: &str,
    id: u32,
    payload: &[u8],
    retries: u32,
    timeout: Duration,
) -> std::io::Result<Vec<u8>> {
    let mut pkt = id.to_be_bytes().to_vec();
    pkt.extend_from_slice(payload);
    sock.set_read_timeout(Some(timeout))?;
    for attempt in 0..=retries {
        sock.send_to(&pkt, addr)?;
        let mut buf = [0u8; 1500];
        match sock.recv_from(&mut buf) {
            Ok((n, _)) if n >= 4 && buf[..4] == id.to_be_bytes() => {
                return Ok(buf[4..n].to_vec());
            }
            Ok(_) => continue, // id distinto, ignorar
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                if attempt == retries {
                    return Err(e);
                }
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

/// Servidor: eco con `drop_rate` (0.0–1.0) simulado por LCG.
pub fn serve(sock: UdpSocket, drop_rate: f64, seed: u64) {
    let mut rng = seed;
    let mut buf = [0u8; 1500];
    loop {
        if let Ok((n, src)) = sock.recv_from(&mut buf) {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let r = (rng >> 33) as f64 / (1u64 << 31) as f64;
            if r < drop_rate {
                continue; // pérdida simulada
            }
            let _ = sock.send_to(&buf[..n], src);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reintenta_ante_perdida() {
        let srv = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = srv.local_addr().unwrap().to_string();
        // 70% de pérdida: forzamos reintentos
        std::thread::spawn(move || serve(srv, 0.7, 1));
        let cli = UdpSocket::bind("127.0.0.1:0").unwrap();
        let t0 = std::time::Instant::now();
        let got = send_reliable(&cli, &addr, 42, b"ping", 20, Duration::from_millis(30)).unwrap();
        assert_eq!(got, b"ping");
        assert!(t0.elapsed() > Duration::from_millis(20), "debió reintentar");
    }
    #[test]
    fn sin_perdida_rapido() {
        let srv = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = srv.local_addr().unwrap().to_string();
        std::thread::spawn(move || serve(srv, 0.0, 1));
        let cli = UdpSocket::bind("127.0.0.1:0").unwrap();
        let got = send_reliable(&cli, &addr, 1, b"hola", 3, Duration::from_millis(200)).unwrap();
        assert_eq!(got, b"hola");
    }
}
