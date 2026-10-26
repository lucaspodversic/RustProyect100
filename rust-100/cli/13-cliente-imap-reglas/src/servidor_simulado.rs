//! Servidor IMAP simulado, en memoria, que habla un **subconjunto** de RFC 3501:
//! LOGIN, SELECT, UID SEARCH (ALL/UNSEEN), UID FETCH (cabeceras From/Subject como
//! literal), UID STORE +FLAGS[.SILENT], UID COPY, EXPUNGE, LOGOUT.
//!
//! Escucha en `127.0.0.1:0` (puerto libre) y atiende conexiones en un hilo.
//! El estado se comparte vía `Arc<Mutex<..>>` para que el test lo inspeccione.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

/// Mensaje almacenado en el servidor.
#[derive(Debug, Clone)]
pub struct Correo {
    pub uid: u32,
    pub de: String,
    pub asunto: String,
    pub banderas: Vec<String>,
}

/// Carpetas → mensajes.
pub type Buzon = Arc<Mutex<BTreeMap<String, Vec<Correo>>>>;

/// Crea un buzón con INBOX y algunas carpetas vacías.
pub fn buzon_demo() -> Buzon {
    let c = |uid, de: &str, asunto: &str| Correo { uid, de: de.into(), asunto: asunto.into(), banderas: vec![] };
    let mut m = BTreeMap::new();
    m.insert(
        "INBOX".to_string(),
        vec![
            c(1, "Facturación <facturacion@luz.example>", "Tu factura de octubre"),
            c(2, "Tienda <news@tienda.example>", "Gran OFERTA de primavera"),
            c(3, "Ana <ana@amigos.example>", "Asado el sábado?"),
            c(4, "Premio <x@spam.example>", "GANASTE un auto"),
        ],
    );
    m.insert("Facturas".to_string(), vec![]);
    Buzon::new(Mutex::new(m))
}

/// Arranca el servidor; devuelve la dirección para conectarse.
/// `usuario`/`clave` esperados en LOGIN.
pub fn iniciar(buzon: Buzon, usuario: &str, clave: &str) -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let dir = l.local_addr().expect("addr");
    let (u, k) = (usuario.to_string(), clave.to_string());
    thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (b, u, k) = (buzon.clone(), u.clone(), k.clone());
            thread::spawn(move || {
                let _ = atender(s, b, &u, &k);
            });
        }
    });
    dir
}

/// Tokeniza respetando comillas (`"a b"` es un token) y paréntesis como parte del token.
fn tokens(s: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut cur = String::new();
    let mut en_comillas = false;
    let mut escape = false;
    for ch in s.chars() {
        if escape {
            cur.push(ch);
            escape = false;
        } else if en_comillas && ch == '\\' {
            escape = true;
        } else if ch == '"' {
            en_comillas = !en_comillas;
        } else if ch == ' ' && !en_comillas {
            if !cur.is_empty() {
                v.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        v.push(cur);
    }
    v
}

/// Parsea un set de UIDs "1,3,5" (sin rangos `n:m`, suficiente para el cliente).
fn uids(s: &str) -> Vec<u32> {
    s.split(',').filter_map(|n| n.parse().ok()).collect()
}

fn atender(s: TcpStream, buzon: Buzon, usuario: &str, clave: &str) -> std::io::Result<()> {
    let mut w = s.try_clone()?;
    let r = BufReader::new(s);
    w.write_all(b"* OK [simulado] IMAP4rev1 listo\r\n")?;
    let mut autenticado = false;
    let mut carpeta: Option<String> = None;
    for linea in r.lines() {
        let linea = linea?;
        let t = tokens(&linea);
        let (Some(tag), Some(cmd)) = (t.first(), t.get(1)) else { continue };
        let cmd = cmd.to_uppercase();
        let mut out = String::new();
        // Comandos que no requieren sesión.
        if cmd == "LOGOUT" {
            w.write_all(format!("* BYE chau\r\n{tag} OK LOGOUT\r\n").as_bytes())?;
            return Ok(());
        }
        if cmd == "LOGIN" {
            if t.get(2).map(String::as_str) == Some(usuario) && t.get(3).map(String::as_str) == Some(clave) {
                autenticado = true;
                out.push_str(&format!("{tag} OK LOGIN hecho\r\n"));
            } else {
                out.push_str(&format!("{tag} NO [AUTHENTICATIONFAILED] credenciales inválidas\r\n"));
            }
            w.write_all(out.as_bytes())?;
            continue;
        }
        if !autenticado {
            w.write_all(format!("{tag} BAD primero LOGIN\r\n").as_bytes())?;
            continue;
        }
        let mut b = buzon.lock().expect("mutex");
        match cmd.as_str() {
            "SELECT" => {
                let nombre = t.get(2).cloned().unwrap_or_default();
                match b.get(&nombre) {
                    Some(v) => {
                        out.push_str(&format!("* {} EXISTS\r\n{tag} OK [READ-WRITE] SELECT\r\n", v.len()));
                        carpeta = Some(nombre);
                    }
                    None => out.push_str(&format!("{tag} NO no existe la carpeta\r\n")),
                }
            }
            "EXPUNGE" => {
                if let Some(v) = carpeta.as_ref().and_then(|c| b.get_mut(c)) {
                    v.retain(|m| !m.banderas.iter().any(|f| f == "\\Deleted"));
                }
                out.push_str(&format!("{tag} OK EXPUNGE\r\n"));
            }
            "UID" => {
                let sub = t.get(2).map(|s| s.to_uppercase()).unwrap_or_default();
                let Some(c) = carpeta.clone() else {
                    w.write_all(format!("{tag} BAD sin carpeta\r\n").as_bytes())?;
                    continue;
                };
                match sub.as_str() {
                    "SEARCH" => {
                        let solo_no_leidos = t.get(3).is_some_and(|x| x.eq_ignore_ascii_case("UNSEEN"));
                        let ids: Vec<String> = b[&c]
                            .iter()
                            .filter(|m| !solo_no_leidos || !m.banderas.iter().any(|f| f == "\\Seen"))
                            .map(|m| m.uid.to_string())
                            .collect();
                        out.push_str(&format!("* SEARCH {}\r\n{tag} OK SEARCH\r\n", ids.join(" ")));
                    }
                    "FETCH" => {
                        let set = uids(t.get(3).map(String::as_str).unwrap_or(""));
                        for (i, m) in b[&c].iter().enumerate() {
                            if set.contains(&m.uid) {
                                let cab = format!("From: {}\r\nSubject: {}\r\n\r\n", m.de, m.asunto);
                                out.push_str(&format!(
                                    "* {} FETCH (UID {} BODY[HEADER.FIELDS (FROM SUBJECT)] {{{}}}\r\n{})\r\n",
                                    i + 1,
                                    m.uid,
                                    cab.len(),
                                    cab
                                ));
                            }
                        }
                        out.push_str(&format!("{tag} OK FETCH\r\n"));
                    }
                    "STORE" => {
                        let set = uids(t.get(3).map(String::as_str).unwrap_or(""));
                        let bandera = t.get(5).map(|f| f.trim_matches(['(', ')']).to_string()).unwrap_or_default();
                        for m in b.get_mut(&c).expect("carpeta").iter_mut().filter(|m| set.contains(&m.uid)) {
                            if !m.banderas.contains(&bandera) {
                                m.banderas.push(bandera.clone());
                            }
                        }
                        out.push_str(&format!("{tag} OK STORE\r\n"));
                    }
                    "COPY" => {
                        let set = uids(t.get(3).map(String::as_str).unwrap_or(""));
                        let dest = t.get(4).cloned().unwrap_or_default();
                        if !b.contains_key(&dest) {
                            out.push_str(&format!("{tag} NO [TRYCREATE] no existe {dest}\r\n"));
                        } else {
                            let copias: Vec<Correo> = b[&c].iter().filter(|m| set.contains(&m.uid)).cloned().collect();
                            let d = b.get_mut(&dest).expect("dest");
                            for mut m in copias {
                                m.uid = d.iter().map(|x| x.uid).max().unwrap_or(0) + 1; // nuevo UID en destino
                                m.banderas.clear();
                                d.push(m);
                            }
                            out.push_str(&format!("{tag} OK COPY\r\n"));
                        }
                    }
                    _ => out.push_str(&format!("{tag} BAD UID {sub} no soportado\r\n")),
                }
            }
            _ => out.push_str(&format!("{tag} BAD comando no soportado\r\n")),
        }
        drop(b);
        w.write_all(out.as_bytes())?;
    }
    Ok(())
}
