//! # Servidor HTTP/1.1 desde cero
//!
//! Parser de requests + construcción de responses sobre `TcpStream`.
//! Sin frameworks. Soporta GET/POST, headers, body con Content-Length,
//! keep-alive opcional y un router por path exacto.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub reason: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        let reason = match status {
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            400 => "Bad Request",
            404 => "Not Found",
            405 => "Method Not Allowed",
            500 => "Internal Server Error",
            _ => "Unknown",
        }
        .into();
        let body = body.into();
        let mut headers = HashMap::new();
        headers.insert("Content-Length".into(), body.len().to_string());
        headers.insert("Content-Type".into(), "text/plain; charset=utf-8".into());
        headers.insert("Connection".into(), "close".into());
        Self { status, reason, headers, body }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = format!("HTTP/1.1 {} {}\r\n", self.status, self.reason).into_bytes();
        for (k, v) in &self.headers {
            out.extend(format!("{k}: {v}\r\n").as_bytes());
        }
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(&self.body);
        out
    }
}

/// Lee una request completa del socket (headers + body).
pub fn parse_request(stream: &mut impl Read) -> Result<Request, String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    // Leemos hasta encontrar \r\n\r\n (fin de headers).
    loop {
        let n = stream.read(&mut tmp).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("conexión cerrada".into());
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if buf.len() > 64 * 1024 {
            return Err("headers demasiado grandes".into());
        }
    }
    let header_end = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    let header_bytes = &buf[..header_end];
    let rest = buf[header_end..].to_vec();
    let text = std::str::from_utf8(header_bytes).map_err(|e| e.to_string())?;
    let mut lines = text.split("\r\n");
    let start = lines.next().ok_or("sin start-line")?;
    let mut parts = start.split_whitespace();
    let method = parts.next().ok_or("method")?.to_string();
    let path = parts.next().ok_or("path")?.to_string();
    let version = parts.next().ok_or("version")?.to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let clen: usize = headers.get("content-length").and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut body = rest;
    while body.len() < clen {
        let n = stream.read(&mut tmp).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(clen);
    Ok(Request { method, path, version, headers, body })
}

pub type Handler = Arc<dyn Fn(&Request) -> Response + Send + Sync>;

#[derive(Default, Clone)]
pub struct Router {
    routes: HashMap<(String, String), Handler>, // (METHOD, path)
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn route(&mut self, method: &str, path: &str, h: Handler) {
        self.routes.insert((method.to_uppercase(), path.into()), h);
    }
    pub fn handle(&self, req: &Request) -> Response {
        if let Some(h) = self.routes.get(&(req.method.to_uppercase(), req.path.clone())) {
            h(req)
        } else if self.routes.keys().any(|(_, p)| p == &req.path) {
            Response::new(405, "método no permitido")
        } else {
            Response::new(404, "no encontrado")
        }
    }
}

pub fn serve(listener: TcpListener, router: Router) {
    let router = Arc::new(router);
    for s in listener.incoming().flatten() {
        let r = router.clone();
        std::thread::spawn(move || {
            let _ = handle_client(s, &r);
        });
    }
}

fn handle_client(mut s: TcpStream, router: &Router) -> Result<(), String> {
    let req = parse_request(&mut s)?;
    let resp = router.handle(&req);
    s.write_all(&resp.encode()).map_err(|e| e.to_string())
}

/// Cliente mínimo para tests.
pub fn request(addr: &str, raw: &str) -> std::io::Result<Vec<u8>> {
    let mut s = TcpStream::connect(addr)?;
    s.write_all(raw.as_bytes())?;
    let mut out = Vec::new();
    s.read_to_end(&mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn get_y_post() {
        let mut r = Router::new();
        r.route("GET", "/hola", Arc::new(|_| Response::new(200, "hola")));
        r.route(
            "POST",
            "/echo",
            Arc::new(|req| Response::new(200, req.body.clone())),
        );
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        std::thread::spawn(move || serve(l, r));
        let got = request(&addr, "GET /hola HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        assert!(got.starts_with(b"HTTP/1.1 200"));
        assert!(got.windows(4).any(|w| w == b"hola"));
        let body = "ping";
        let raw = format!(
            "POST /echo HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let got = request(&addr, &raw).unwrap();
        assert!(got.ends_with(b"ping"));
        let got = request(&addr, "GET /nope HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        assert!(got.starts_with(b"HTTP/1.1 404"));
    }
}
