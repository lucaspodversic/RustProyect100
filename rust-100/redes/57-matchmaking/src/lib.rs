//! # Matchmaking por WebSockets
//!
//! Handshake HTTP Upgrade + frames texto. Los jugadores envían
//! `{"action":"queue","game":"chess","elo":1200}` y el servidor los
//! empareja por juego y ELO cercano (±200).

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// SHA-1 mínimo (mismo que en mini-git) para la clave Sec-WebSocket-Accept.
mod sha1 {
    pub fn hash(data: &[u8]) -> [u8; 20] {
        fn f(t: u32, b: u32, c: u32, d: u32) -> u32 {
            match t { 0..=19 => (b & c) | ((!b) & d), 20..=39 | 60..=79 => b ^ c ^ d, _ => (b & c) | (b & d) | (c & d) }
        }
        fn k(t: u32) -> u32 { match t { 0..=19 => 0x5A827999, 20..=39 => 0x6ED9EBA1, 40..=59 => 0x8F1BBCDC, _ => 0xCA62C1D6 } }
        let mut msg = data.to_vec();
        let bit_len = (data.len() as u64) * 8;
        msg.push(0x80);
        while msg.len() % 64 != 56 { msg.push(0); }
        msg.extend_from_slice(&bit_len.to_be_bytes());
        let mut h = [0x67452301u32, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
        for chunk in msg.chunks(64) {
            let mut w = [0u32; 80];
            for i in 0..16 { w[i] = u32::from_be_bytes(chunk[i*4..i*4+4].try_into().unwrap()); }
            for t in 16..80 { w[t] = (w[t-3] ^ w[t-8] ^ w[t-14] ^ w[t-16]).rotate_left(1); }
            let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
            for t in 0..80 {
                let temp = a.rotate_left(5).wrapping_add(f(t as u32, b, c, d)).wrapping_add(e).wrapping_add(k(t as u32)).wrapping_add(w[t]);
                e = d; d = c; c = b.rotate_left(30); b = a; a = temp;
            }
            for i in 0..5 { h[i] = h[i].wrapping_add([a, b, c, d, e][i]); }
        }
        let mut out = [0u8; 20];
        for (i, v) in h.iter().enumerate() { out[i*4..i*4+4].copy_from_slice(&v.to_be_bytes()); }
        out
    }
}

mod b64 {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    pub fn encode(data: &[u8]) -> String {
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let mut n = (chunk[0] as u32) << 16;
            if chunk.len() > 1 { n |= (chunk[1] as u32) << 8; }
            if chunk.len() > 2 { n |= chunk[2] as u32; }
            out.push(T[((n >> 18) & 63) as usize] as char);
            out.push(T[((n >> 12) & 63) as usize] as char);
            out.push(if chunk.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
            out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
        }
        out
    }
}

pub fn ws_accept_key(client_key: &str) -> String {
    const MAGIC: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let mut data = client_key.as_bytes().to_vec();
    data.extend_from_slice(MAGIC.as_bytes());
    b64::encode(&sha1::hash(&data))
}

pub fn handshake(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let key = req.lines()
        .find_map(|l| l.strip_prefix("Sec-WebSocket-Key:").map(|s| s.trim().to_string()))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "sin key"))?;
    let accept = ws_accept_key(&key);
    let resp = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    stream.write_all(resp.as_bytes())
}

/// Lee un frame texto (cliente→servidor, enmascarado).
pub fn read_text(stream: &mut TcpStream) -> std::io::Result<String> {
    let mut hdr = [0u8; 2];
    stream.read_exact(&mut hdr)?;
    let opcode = hdr[0] & 0x0F;
    let masked = hdr[1] & 0x80 != 0;
    let mut len = (hdr[1] & 0x7F) as usize;
    if len == 126 {
        let mut e = [0u8; 2]; stream.read_exact(&mut e)?;
        len = u16::from_be_bytes(e) as usize;
    } else if len == 127 {
        let mut e = [0u8; 8]; stream.read_exact(&mut e)?;
        len = u64::from_be_bytes(e) as usize;
    }
    let mut mask = [0u8; 4];
    if masked { stream.read_exact(&mut mask)?; }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload)?;
    if masked {
        for (i, b) in payload.iter_mut().enumerate() { *b ^= mask[i % 4]; }
    }
    match opcode {
        0x1 => String::from_utf8(payload).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        0x8 => Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "close")),
        0x9 => { write_frame(stream, 0xA, &payload)?; read_text(stream) } // pong + retry
        _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "opcode")),
    }
}

pub fn write_frame(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut out = Vec::new();
    out.push(0x80 | opcode);
    if payload.len() < 126 {
        out.push(payload.len() as u8);
    } else if payload.len() < 65536 {
        out.push(126);
        out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        out.push(127);
        out.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }
    out.extend_from_slice(payload);
    stream.write_all(&out)
}

pub fn write_text(stream: &mut TcpStream, text: &str) -> std::io::Result<()> {
    write_frame(stream, 0x1, text.as_bytes())
}

#[derive(Debug, Clone)]
pub struct Player {
    pub id: String,
    pub game: String,
    pub elo: i32,
}

#[derive(Debug, Default)]
pub struct Matchmaker {
    queues: HashMap<String, VecDeque<Player>>,
}

impl Matchmaker {
    pub fn enqueue(&mut self, p: Player) -> Option<(Player, Player)> {
        let q = self.queues.entry(p.game.clone()).or_default();
        // Buscar rival con |elo_diff| <= 200
        if let Some(i) = q.iter().position(|o| (o.elo - p.elo).abs() <= 200) {
            let other = q.remove(i).unwrap();
            return Some((other, p));
        }
        q.push_back(p);
        None
    }
    pub fn len(&self, game: &str) -> usize {
        self.queues.get(game).map(|q| q.len()).unwrap_or(0)
    }
}

/// Extrae campos simples de un JSON plano.
fn jget(s: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":");
    let i = s.find(&pat)? + pat.len();
    let rest = s[i..].trim_start();
    if rest.starts_with('"') {
        let end = rest[1..].find('"')? + 1;
        Some(rest[1..end].to_string())
    } else {
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        Some(rest[..end].trim().to_string())
    }
}

pub fn serve(listener: TcpListener, mm: Arc<Mutex<Matchmaker>>) {
    // Mapa id → socket para notificar el match.
    let sockets: Arc<Mutex<HashMap<String, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));
    for s in listener.incoming().flatten() {
        let mm = mm.clone();
        let sockets = sockets.clone();
        std::thread::spawn(move || {
            let mut s = s;
            if handshake(&mut s).is_err() { return; }
            loop {
                let Ok(text) = read_text(&mut s) else { break; };
                let action = jget(&text, "action").unwrap_or_default();
                if action == "queue" {
                    let id = jget(&text, "id").unwrap_or_else(|| format!("p{}", std::process::id()));
                    let game = jget(&text, "game").unwrap_or_else(|| "default".into());
                    let elo: i32 = jget(&text, "elo").and_then(|s| s.parse().ok()).unwrap_or(1000);
                    sockets.lock().unwrap().insert(id.clone(), s.try_clone().unwrap());
                    let matched = mm.lock().unwrap().enqueue(Player { id: id.clone(), game, elo });
                    if let Some((a, b)) = matched {
                        let msg_a = format!(r#"{{"event":"match","opponent":"{}","elo":{}}}"#, b.id, b.elo);
                        let msg_b = format!(r#"{{"event":"match","opponent":"{}","elo":{}}}"#, a.id, a.elo);
                        if let Some(sa) = sockets.lock().unwrap().get_mut(&a.id) { let _ = write_text(sa, &msg_a); }
                        if let Some(sb) = sockets.lock().unwrap().get_mut(&b.id) { let _ = write_text(sb, &msg_b); }
                    } else {
                        let _ = write_text(&mut s, r#"{"event":"queued"}"#);
                    }
                }
            }
        });
    }
}

/// Cliente WS mínimo (con máscara).
pub struct WsClient { stream: TcpStream }
impl WsClient {
    pub fn connect(addr: &str) -> std::io::Result<Self> {
        let mut stream = TcpStream::connect(addr)?;
        let key = b64::encode(b"0123456789abcdef");
        let req = format!(
            "GET / HTTP/1.1\r\nHost: {addr}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        stream.write_all(req.as_bytes())?;
        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf)?;
        if !String::from_utf8_lossy(&buf[..n]).contains("101") {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "handshake falló"));
        }
        Ok(Self { stream })
    }
    pub fn send(&mut self, text: &str) -> std::io::Result<()> {
        let payload = text.as_bytes();
        let mut out = Vec::new();
        out.push(0x81);
        out.push(0x80 | payload.len() as u8); // masked, len < 126
        let mask = [1u8, 2, 3, 4];
        out.extend_from_slice(&mask);
        for (i, &b) in payload.iter().enumerate() { out.push(b ^ mask[i % 4]); }
        self.stream.write_all(&out)
    }
    pub fn recv(&mut self) -> std::io::Result<String> {
        self.stream.set_read_timeout(Some(std::time::Duration::from_millis(500)))?;
        read_text(&mut self.stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accept_key_rfc() {
        // Ejemplo del RFC 6455
        assert_eq!(ws_accept_key("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }
    #[test]
    fn match_por_elo() {
        let mut mm = Matchmaker::default();
        assert!(mm.enqueue(Player { id: "a".into(), game: "chess".into(), elo: 1200 }).is_none());
        let m = mm.enqueue(Player { id: "b".into(), game: "chess".into(), elo: 1300 });
        assert!(m.is_some());
        assert!(mm.enqueue(Player { id: "c".into(), game: "chess".into(), elo: 2000 }).is_none());
        assert!(mm.enqueue(Player { id: "d".into(), game: "chess".into(), elo: 1000 }).is_none()); // lejos de 2000
    }
    #[test]
    fn ws_end_to_end() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        std::thread::spawn(move || serve(l, Arc::new(Mutex::new(Matchmaker::default()))));
        std::thread::sleep(std::time::Duration::from_millis(30));
        let mut a = WsClient::connect(&addr).unwrap();
        let mut b = WsClient::connect(&addr).unwrap();
        a.send(r#"{"action":"queue","id":"a","game":"go","elo":1500}"#).unwrap();
        assert!(a.recv().unwrap().contains("queued"));
        b.send(r#"{"action":"queue","id":"b","game":"go","elo":1550}"#).unwrap();
        let ra = a.recv().unwrap();
        let rb = b.recv().unwrap();
        assert!(ra.contains("match") && ra.contains("b"));
        assert!(rb.contains("match") && rb.contains("a"));
    }
}
