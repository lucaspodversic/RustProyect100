//! # RPC con serialización binaria (bincode)
//!
//! Framing: `[u32 LE longitud][payload bincode]`. Request/Response tipados
//! con serde. Servidor despacha por nombre de método.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcRequest {
    pub id: u64,
    pub method: String,
    pub params: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcResponse {
    pub id: u64,
    pub ok: bool,
    pub payload: Vec<u8>,
}

pub fn write_msg<W: Write, T: Serialize>(w: &mut W, msg: &T) -> std::io::Result<()> {
    let bytes = bincode::serialize(msg).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    w.write_all(&(bytes.len() as u32).to_le_bytes())?;
    w.write_all(&bytes)?;
    w.flush()
}

pub fn read_msg<R: Read, T: for<'de> Deserialize<'de>>(r: &mut R) -> std::io::Result<T> {
    let mut lenb = [0u8; 4];
    r.read_exact(&mut lenb)?;
    let len = u32::from_le_bytes(lenb) as usize;
    if len > 1_000_000 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "msg demasiado grande"));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    bincode::deserialize(&buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub type Handler = Box<dyn Fn(&[u8]) -> Result<Vec<u8>, String> + Send + Sync>;

pub struct RpcServer {
    methods: HashMap<String, Handler>,
}

impl RpcServer {
    pub fn new() -> Self { Self { methods: HashMap::new() } }

    pub fn register<F>(&mut self, name: &str, f: F)
    where
        F: Fn(&[u8]) -> Result<Vec<u8>, String> + Send + Sync + 'static,
    {
        self.methods.insert(name.into(), Box::new(f));
    }

    pub fn handle(&self, req: RpcRequest) -> RpcResponse {
        match self.methods.get(&req.method) {
            Some(h) => match h(&req.params) {
                Ok(payload) => RpcResponse { id: req.id, ok: true, payload },
                Err(e) => RpcResponse { id: req.id, ok: false, payload: e.into_bytes() },
            },
            None => RpcResponse {
                id: req.id,
                ok: false,
                payload: format!("método desconocido: {}", req.method).into_bytes(),
            },
        }
    }

    pub fn serve(self: Arc<Self>, listener: TcpListener) {
        for s in listener.incoming().flatten() {
            let srv = self.clone();
            std::thread::spawn(move || {
                let mut s = s;
                while let Ok(req) = read_msg::<_, RpcRequest>(&mut s) {
                    let resp = srv.handle(req);
                    if write_msg(&mut s, &resp).is_err() { break; }
                }
            });
        }
    }
}

pub struct RpcClient {
    stream: TcpStream,
    next_id: u64,
}

impl RpcClient {
    pub fn connect(addr: &str) -> std::io::Result<Self> {
        Ok(Self { stream: TcpStream::connect(addr)?, next_id: 1 })
    }

    pub fn call<P: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: &P,
    ) -> std::io::Result<R> {
        let id = self.next_id;
        self.next_id += 1;
        let params = bincode::serialize(params)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        write_msg(&mut self.stream, &RpcRequest { id, method: method.into(), params })?;
        let resp: RpcResponse = read_msg(&mut self.stream)?;
        if resp.id != id {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "id mismatch"));
        }
        if !resp.ok {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                String::from_utf8_lossy(&resp.payload).to_string(),
            ));
        }
        bincode::deserialize(&resp.payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suma_remota() {
        let mut srv = RpcServer::new();
        srv.register("add", |params| {
            let (a, b): (i32, i32) = bincode::deserialize(params).map_err(|e| e.to_string())?;
            bincode::serialize(&(a + b)).map_err(|e| e.to_string())
        });
        srv.register("echo", |p| Ok(p.to_vec()));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let srv = Arc::new(srv);
        std::thread::spawn(move || srv.serve(listener));
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut cli = RpcClient::connect(&addr).unwrap();
        let sum: i32 = cli.call("add", &(20i32, 22i32)).unwrap();
        assert_eq!(sum, 42);
        let echo: String = {
            let s: String = "hola".into();
            // echo returns raw bytes of serialized String
            let raw: Vec<u8> = cli.call("echo", &s).unwrap();
            // wait - call deserializes payload as R; for echo we want Vec<u8> then decode
            let _ = raw;
            s
        };
        // better: call echo returning the same string via params roundtrip through handler
        let mut cli2 = RpcClient::connect(&addr).unwrap();
        // re-register path: echo returns params as-is which is bincode of String
        let got: String = {
            // We need a method that returns typed String. Use add path style:
            // call with custom: serialize params as String, handler returns same bytes,
            // client deserializes as String.
            cli2.call("echo", &"mundo".to_string()).unwrap()
        };
        assert_eq!(got, "mundo");
        let err = cli2.call::<(i32, i32), i32>("nope", &(1, 2));
        assert!(err.is_err());
        let _ = echo;
    }
}
