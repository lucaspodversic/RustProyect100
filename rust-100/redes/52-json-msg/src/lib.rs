//! # Protocolo de mensajería propio (JSON-over-TCP)
//!
//! Framing: `[u32 BE length][utf-8 JSON]`.
//! Mensajes: `{"type":"hello","name":"..."}`, `{"type":"chat","text":"..."}`,
//! `{"type":"ping"}`, `{"type":"pong"}`, `{"type":"bye"}`.
//! Encoder/decoder JSON mínimo (sin serde) para el esquema fijo.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    Hello { name: String },
    Chat { text: String },
    Ping,
    Pong,
    Bye,
}

impl Msg {
    pub fn encode(&self) -> String {
        match self {
            Msg::Hello { name } => format!(r#"{{"type":"hello","name":"{}"}}"#, escape(name)),
            Msg::Chat { text } => format!(r#"{{"type":"chat","text":"{}"}}"#, escape(text)),
            Msg::Ping => r#"{"type":"ping"}"#.into(),
            Msg::Pong => r#"{"type":"pong"}"#.into(),
            Msg::Bye => r#"{"type":"bye"}"#.into(),
        }
    }
    pub fn decode(s: &str) -> Result<Msg, String> {
        let t = extract(s, "type").ok_or("sin type")?;
        match t.as_str() {
            "hello" => Ok(Msg::Hello { name: extract(s, "name").unwrap_or_default() }),
            "chat" => Ok(Msg::Chat { text: extract(s, "text").unwrap_or_default() }),
            "ping" => Ok(Msg::Ping),
            "pong" => Ok(Msg::Pong),
            "bye" => Ok(Msg::Bye),
            o => Err(format!("type desconocido: {o}")),
        }
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Extrae el valor string de `"key":"..."` (suficiente para nuestro esquema).
fn extract(json: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":\"");
    let i = json.find(&pat)? + pat.len();
    let mut out = String::new();
    let mut chars = json[i..].chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(chars.next()?),
            '"' => break,
            c => out.push(c),
        }
    }
    Some(out)
}

pub fn write_msg(w: &mut impl Write, m: &Msg) -> std::io::Result<()> {
    let body = m.encode().into_bytes();
    w.write_all(&(body.len() as u32).to_be_bytes())?;
    w.write_all(&body)
}

pub fn read_msg(r: &mut impl Read) -> std::io::Result<Msg> {
    let mut lenb = [0u8; 4];
    r.read_exact(&mut lenb)?;
    let len = u32::from_be_bytes(lenb) as usize;
    if len > 1_000_000 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "mensaje enorme"));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    let s = String::from_utf8(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Msg::decode(&s).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub fn serve(listener: TcpListener, log: Arc<Mutex<Vec<Msg>>>) {
    for s in listener.incoming().flatten() {
        let log = log.clone();
        std::thread::spawn(move || {
            let mut s = s;
            loop {
                match read_msg(&mut s) {
                    Ok(Msg::Ping) => { let _ = write_msg(&mut s, &Msg::Pong); }
                    Ok(Msg::Bye) => break,
                    Ok(m) => {
                        log.lock().unwrap().push(m.clone());
                        if let Msg::Chat { text } = &m {
                            let _ = write_msg(&mut s, &Msg::Chat { text: format!("eco:{text}") });
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_codec() {
        let m = Msg::Hello { name: "Ada \"Lovelace\"".into() };
        assert_eq!(Msg::decode(&m.encode()).unwrap(), m);
    }
    #[test]
    fn servidor() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let log = Arc::new(Mutex::new(Vec::new()));
        let log2 = log.clone();
        std::thread::spawn(move || serve(l, log2));
        let mut s = TcpStream::connect(&addr).unwrap();
        write_msg(&mut s, &Msg::Hello { name: "bob".into() }).unwrap();
        write_msg(&mut s, &Msg::Ping).unwrap();
        assert_eq!(read_msg(&mut s).unwrap(), Msg::Pong);
        write_msg(&mut s, &Msg::Chat { text: "hola".into() }).unwrap();
        match read_msg(&mut s).unwrap() {
            Msg::Chat { text } => assert_eq!(text, "eco:hola"),
            o => panic!("{o:?}"),
        }
        write_msg(&mut s, &Msg::Bye).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let g = log.lock().unwrap();
        assert!(g.iter().any(|m| matches!(m, Msg::Hello { .. })));
    }
}
