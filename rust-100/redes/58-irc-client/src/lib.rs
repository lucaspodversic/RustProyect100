//! # Cliente IRC básico
//!
//! Habla el protocolo de texto IRC (RFC 1459 subset): NICK, USER, JOIN,
//! PRIVMSG, PING/PONG. Los tests levantan un servidor mock.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Connected,
    Motd(String),
    Join { channel: String },
    Message { from: String, target: String, text: String },
    Ping(String),
    Notice(String),
    Other(String),
}

pub struct IrcClient {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    pub nick: String,
}

impl IrcClient {
    pub fn connect(addr: &str, nick: &str, user: &str) -> std::io::Result<Self> {
        let w = TcpStream::connect(addr)?;
        let mut c = Self {
            reader: BufReader::new(w.try_clone()?),
            writer: w,
            nick: nick.into(),
        };
        c.raw(&format!("NICK {nick}"))?;
        c.raw(&format!("USER {user} 0 * :{user}"))?;
        Ok(c)
    }

    pub fn raw(&mut self, line: &str) -> std::io::Result<()> {
        self.writer.write_all(format!("{line}\r\n").as_bytes())
    }

    pub fn join(&mut self, channel: &str) -> std::io::Result<()> {
        self.raw(&format!("JOIN {channel}"))
    }

    pub fn privmsg(&mut self, target: &str, text: &str) -> std::io::Result<()> {
        self.raw(&format!("PRIVMSG {target} :{text}"))
    }

    pub fn quit(&mut self, msg: &str) -> std::io::Result<()> {
        self.raw(&format!("QUIT :{msg}"))
    }

    /// Lee y parsea una línea. Responde PONG automáticamente.
    pub fn next_event(&mut self) -> std::io::Result<Event> {
        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        let line = line.trim_end_matches(['\r', '\n']).to_string();
        if line.is_empty() {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        let ev = parse_line(&line);
        if let Event::Ping(token) = &ev {
            self.raw(&format!("PONG :{token}"))?;
        }
        Ok(ev)
    }
}

pub fn parse_line(line: &str) -> Event {
    // [:prefix] COMMAND args... [:trailing]
    let (prefix, rest) = if let Some(r) = line.strip_prefix(':') {
        let (p, r) = r.split_once(' ').unwrap_or((r, ""));
        (Some(p), r)
    } else {
        (None, line)
    };
    let mut parts = rest.splitn(2, ':');
    let front = parts.next().unwrap_or("").trim();
    let trailing = parts.next().unwrap_or("").to_string();
    let mut toks = front.split_whitespace();
    let cmd = toks.next().unwrap_or("");
    match cmd {
        "PING" => Event::Ping(if trailing.is_empty() {
            toks.next().unwrap_or("").to_string()
        } else {
            trailing
        }),
        "PRIVMSG" => {
            let target = toks.next().unwrap_or("").to_string();
            let from = prefix.unwrap_or("").split('!').next().unwrap_or("").to_string();
            Event::Message { from, target, text: trailing }
        }
        "JOIN" => Event::Join {
            channel: if trailing.is_empty() { toks.next().unwrap_or("").into() } else { trailing },
        },
        "001" | "002" | "003" | "004" => Event::Connected,
        "372" | "375" | "376" => Event::Motd(trailing),
        "NOTICE" => Event::Notice(trailing),
        _ => Event::Other(line.to_string()),
    }
}

/// Servidor mock mínimo para tests.
pub fn mock_server(listener: std::net::TcpListener) {
    for s in listener.incoming().flatten() {
        std::thread::spawn(move || {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut w = s;
            let mut nick = String::from("guest");
            let mut line = String::new();
            loop {
                line.clear();
                if r.read_line(&mut line).unwrap_or(0) == 0 { break; }
                let line = line.trim_end_matches(['\r', '\n']).to_string();
                if let Some(n) = line.strip_prefix("NICK ") {
                    nick = n.to_string();
                    let _ = write!(w, ":mock 001 {nick} :Welcome\r\n");
                } else if line.starts_with("USER ") {
                    // ignore
                } else if let Some(ch) = line.strip_prefix("JOIN ") {
                    let _ = write!(w, ":{nick}!u@h JOIN :{ch}\r\n");
                    let _ = write!(w, ":other!u@h PRIVMSG {ch} :hola {nick}\r\n");
                } else if line.starts_with("PRIVMSG ") {
                    // eco como NOTICE
                    let _ = write!(w, ":mock NOTICE {nick} :ok\r\n");
                } else if line.starts_with("PONG ") {
                    // silencioso
                } else if line.starts_with("QUIT") {
                    break;
                }
                // Empujar un PING ocasional tras NICK
                if line.starts_with("NICK ") {
                    let _ = write!(w, "PING :xyz\r\n");
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_privmsg() {
        let e = parse_line(":alice!a@h PRIVMSG #dev :hola mundo");
        assert_eq!(e, Event::Message { from: "alice".into(), target: "#dev".into(), text: "hola mundo".into() });
    }
    #[test]
    fn cliente_contra_mock() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        std::thread::spawn(move || mock_server(l));
        let mut c = IrcClient::connect(&addr, "rustie", "rustie").unwrap();
        // 001
        assert!(matches!(c.next_event().unwrap(), Event::Connected));
        // PING → auto PONG
        assert!(matches!(c.next_event().unwrap(), Event::Ping(_)));
        c.join("#rust").unwrap();
        // Puede haber eventos intermedios; buscamos JOIN y luego PRIVMSG.
        let mut saw_join = false;
        let mut saw_msg = false;
        for _ in 0..8 {
            match c.next_event().unwrap() {
                Event::Join { channel } if channel == "#rust" => saw_join = true,
                Event::Message { from, text, .. } => {
                    assert_eq!(from, "other");
                    assert!(text.contains("rustie"));
                    saw_msg = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(saw_join && saw_msg);
        c.privmsg("#rust", "ping").unwrap();
        let mut ok = false;
        for _ in 0..4 {
            if matches!(c.next_event().unwrap(), Event::Notice(s) if s == "ok") { ok = true; break; }
        }
        assert!(ok);
    }
}
