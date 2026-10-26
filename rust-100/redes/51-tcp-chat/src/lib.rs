//! # Chat TCP multihilo
//!
//! Usuarios con nick, salas, broadcast. Protocolo de texto:
//! `/nick N`, `/join SALA`, `/leave`, `/msg texto`, `/who`, `/quit`.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct Hub {
    /// nick → writer (clon del socket)
    pub users: HashMap<String, TcpStream>,
    /// sala → nicks
    pub rooms: HashMap<String, HashSet<String>>,
    /// nick → sala actual
    pub where_is: HashMap<String, String>,
}

impl Hub {
    pub fn broadcast(&mut self, room: &str, from: &str, msg: &str) {
        let line = format!("[{room}] {from}: {msg}\n");
        if let Some(members) = self.rooms.get(room).cloned() {
            for m in members {
                if let Some(w) = self.users.get_mut(&m) {
                    let _ = w.write_all(line.as_bytes());
                }
            }
        }
    }

    pub fn send_to(&mut self, nick: &str, msg: &str) {
        if let Some(w) = self.users.get_mut(nick) {
            let _ = w.write_all(msg.as_bytes());
        }
    }
}

pub fn handle_client(hub: Arc<Mutex<Hub>>, mut stream: TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut nick = format!("anon-{}", stream.peer_addr().map(|a| a.port()).unwrap_or(0));
    {
        let mut h = hub.lock().unwrap();
        h.users.insert(nick.clone(), stream.try_clone().unwrap());
        h.send_to(&nick, &format!("bienvenido. tu nick es {nick}. comandos: /nick /join /leave /msg /who /quit\n"));
    }
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim_end();
        let mut h = hub.lock().unwrap();
        if let Some(n) = line.strip_prefix("/nick ") {
            let n = n.trim();
            if n.is_empty() || h.users.contains_key(n) {
                h.send_to(&nick, "ERR nick inválido o tomado\n");
                continue;
            }
            // renombrar
            if let Some(sock) = h.users.remove(&nick) {
                h.users.insert(n.into(), sock);
            }
            if let Some(room) = h.where_is.remove(&nick) {
                if let Some(set) = h.rooms.get_mut(&room) {
                    set.remove(&nick);
                    set.insert(n.into());
                }
                h.where_is.insert(n.into(), room);
            }
            nick = n.into();
            h.send_to(&nick, &format!("OK nick={nick}\n"));
        } else if let Some(room) = line.strip_prefix("/join ") {
            let room = room.trim().to_string();
            if let Some(old) = h.where_is.remove(&nick) {
                if let Some(set) = h.rooms.get_mut(&old) { set.remove(&nick); }
            }
            h.rooms.entry(room.clone()).or_default().insert(nick.clone());
            h.where_is.insert(nick.clone(), room.clone());
            h.send_to(&nick, &format!("OK joined {room}\n"));
            h.broadcast(&room, "*", &format!("{nick} entró"));
        } else if line == "/leave" {
            if let Some(room) = h.where_is.remove(&nick) {
                if let Some(set) = h.rooms.get_mut(&room) { set.remove(&nick); }
                h.broadcast(&room, "*", &format!("{nick} salió"));
                h.send_to(&nick, "OK left\n");
            }
        } else if let Some(msg) = line.strip_prefix("/msg ") {
            if let Some(room) = h.where_is.get(&nick).cloned() {
                h.broadcast(&room, &nick, msg);
            } else {
                h.send_to(&nick, "ERR no estás en ninguna sala\n");
            }
        } else if line == "/who" {
            if let Some(room) = h.where_is.get(&nick) {
                let members: Vec<_> = h.rooms.get(room).map(|s| s.iter().cloned().collect()).unwrap_or_default();
                h.send_to(&nick, &format!("WHO {}\n", members.join(", ")));
            }
        } else if line == "/quit" {
            break;
        } else if !line.is_empty() {
            // mensaje corto = /msg
            if let Some(room) = h.where_is.get(&nick).cloned() {
                h.broadcast(&room, &nick, line);
            } else {
                h.send_to(&nick, "ERR usá /join primero\n");
            }
        }
    }
    // cleanup
    let mut h = hub.lock().unwrap();
    h.users.remove(&nick);
    if let Some(room) = h.where_is.remove(&nick) {
        if let Some(set) = h.rooms.get_mut(&room) { set.remove(&nick); }
        h.broadcast(&room, "*", &format!("{nick} desconectado"));
    }
}

pub fn serve(listener: TcpListener, hub: Arc<Mutex<Hub>>) {
    for s in listener.incoming().flatten() {
        let h = hub.clone();
        std::thread::spawn(move || handle_client(h, s));
    }
}

/// Cliente de test: envía líneas y lee respuestas con timeout corto.
pub struct ChatClient {
    r: BufReader<TcpStream>,
    w: TcpStream,
}
impl ChatClient {
    pub fn connect(addr: &str) -> std::io::Result<Self> {
        let w = TcpStream::connect(addr)?;
        w.set_read_timeout(Some(std::time::Duration::from_millis(300)))?;
        Ok(Self { r: BufReader::new(w.try_clone()?), w })
    }
    pub fn send(&mut self, line: &str) -> std::io::Result<()> {
        self.w.write_all(format!("{line}\n").as_bytes())
    }
    pub fn recv_line(&mut self) -> Option<String> {
        let mut s = String::new();
        match self.r.read_line(&mut s) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(s.trim_end().to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dos_usuarios_en_sala() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        std::thread::spawn(move || serve(l, Arc::new(Mutex::new(Hub::default()))));
        let mut a = ChatClient::connect(&addr).unwrap();
        let mut b = ChatClient::connect(&addr).unwrap();
        let _ = a.recv_line(); // bienvenida
        let _ = b.recv_line();
        a.send("/nick alice").unwrap();
        assert!(a.recv_line().unwrap().contains("alice"));
        b.send("/nick bob").unwrap();
        let _ = b.recv_line();
        a.send("/join general").unwrap();
        let _ = a.recv_line(); // OK joined
        let _ = a.recv_line(); // broadcast entró
        b.send("/join general").unwrap();
        let _ = b.recv_line();
        let _ = b.recv_line();
        let _ = a.recv_line(); // alice ve a bob entrar
        a.send("/msg hola bob").unwrap();
        let msg = b.recv_line().unwrap();
        assert!(msg.contains("alice") && msg.contains("hola bob"));
    }
}
