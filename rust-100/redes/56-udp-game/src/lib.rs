//! # Protocolo de juego sobre UDP
//!
//! Sincronización de estado: cada paquete es
//! `[seq u32][ack u32][x f32][y f32][vx f32][vy f32]`.
//! El receptor aplica el estado sólo si `seq` es más nuevo (descarta viejos)
//! y trackea pérdida estimada.

use std::net::UdpSocket;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct State {
    pub seq: u32,
    pub ack: u32,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
}

impl State {
    pub fn encode(&self) -> [u8; 24] {
        let mut b = [0u8; 24];
        b[0..4].copy_from_slice(&self.seq.to_be_bytes());
        b[4..8].copy_from_slice(&self.ack.to_be_bytes());
        b[8..12].copy_from_slice(&self.x.to_be_bytes());
        b[12..16].copy_from_slice(&self.y.to_be_bytes());
        b[16..20].copy_from_slice(&self.vx.to_be_bytes());
        b[20..24].copy_from_slice(&self.vy.to_be_bytes());
        b
    }
    pub fn decode(b: &[u8]) -> Option<Self> {
        if b.len() < 24 { return None; }
        Some(Self {
            seq: u32::from_be_bytes(b[0..4].try_into().ok()?),
            ack: u32::from_be_bytes(b[4..8].try_into().ok()?),
            x: f32::from_be_bytes(b[8..12].try_into().ok()?),
            y: f32::from_be_bytes(b[12..16].try_into().ok()?),
            vx: f32::from_be_bytes(b[16..20].try_into().ok()?),
            vy: f32::from_be_bytes(b[20..24].try_into().ok()?),
        })
    }
}

#[derive(Debug, Default)]
pub struct Peer {
    pub state: State,
    pub last_seq: u32,
    pub received: u64,
    pub expected: u64,
}

impl Peer {
    /// Aplica un paquete si es más nuevo. Actualiza stats de pérdida.
    pub fn apply(&mut self, s: State) -> bool {
        self.received += 1;
        if s.seq <= self.last_seq && self.last_seq != 0 {
            return false; // viejo / duplicado
        }
        if self.last_seq != 0 {
            self.expected += (s.seq - self.last_seq) as u64;
        } else {
            self.expected = 1;
        }
        self.last_seq = s.seq;
        self.state = s;
        true
    }

    pub fn loss_rate(&self) -> f64 {
        if self.expected == 0 { return 0.0; }
        1.0 - (self.received as f64 / self.expected as f64)
    }
}

pub fn send(sock: &UdpSocket, addr: &str, s: &State) -> std::io::Result<()> {
    sock.send_to(&s.encode(), addr)?;
    Ok(())
}

pub fn recv(sock: &UdpSocket) -> std::io::Result<(State, String)> {
    let mut buf = [0u8; 64];
    let (n, src) = sock.recv_from(&mut buf)?;
    let s = State::decode(&buf[..n]).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "pkt"))?;
    Ok((s, src.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn sync_y_descarta_viejos() {
        let a = UdpSocket::bind("127.0.0.1:0").unwrap();
        let b = UdpSocket::bind("127.0.0.1:0").unwrap();
        a.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
        b.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
        let addr_b = b.local_addr().unwrap().to_string();
        let mut peer = Peer::default();
        for seq in [1u32, 2, 3] {
            send(&a, &addr_b, &State { seq, ack: 0, x: seq as f32, y: 0.0, vx: 1.0, vy: 0.0 }).unwrap();
            let (s, _) = recv(&b).unwrap();
            assert!(peer.apply(s));
        }
        assert_eq!(peer.state.x, 3.0);
        // Paquete viejo
        assert!(!peer.apply(State { seq: 2, ack: 0, x: 99.0, y: 0.0, vx: 0.0, vy: 0.0 }));
        assert_eq!(peer.state.x, 3.0);
        // Salto (pérdida de 4 y 5)
        assert!(peer.apply(State { seq: 6, ack: 0, x: 6.0, y: 0.0, vx: 0.0, vy: 0.0 }));
        assert!(peer.loss_rate() > 0.0);
    }
}
