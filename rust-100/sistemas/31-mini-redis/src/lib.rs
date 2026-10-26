//! # Mini Redis
//!
//! Almacén clave-valor compatible con un subconjunto del protocolo **RESP2**
//! (REdis Serialization Protocol). Incluye:
//!
//! - `resp`: parser y serializador de valores RESP (incremental, sobre bytes).
//! - `store`: el almacén en memoria con expiración (TTL) por clave.
//! - `persist`: persistencia tipo "snapshot" (similar a RDB) a un archivo.
//! - `server`: servidor TCP multihilo (un hilo por conexión).
//! - `client`: cliente mínimo usado por la CLI y por los tests.
//!
//! Todo está hecho con la biblioteca estándar para que sea fácil de leer.

pub mod resp {
    //! Tipos y codificación del protocolo RESP2.

    /// Un valor RESP. Redis define 5 tipos en RESP2.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Value {
        /// `+OK\r\n` — cadena simple (sin saltos de línea).
        Simple(String),
        /// `-ERR mensaje\r\n` — error.
        Error(String),
        /// `:42\r\n` — entero con signo de 64 bits.
        Int(i64),
        /// `$5\r\nhola!\r\n` — cadena binaria; `None` representa `$-1` (nil).
        Bulk(Option<Vec<u8>>),
        /// `*2\r\n...` — arreglo de valores; `None` es el arreglo nulo `*-1`.
        Array(Option<Vec<Value>>),
    }

    impl Value {
        /// Atajo para construir un bulk string a partir de texto.
        pub fn bulk(s: impl AsRef<[u8]>) -> Value {
            Value::Bulk(Some(s.as_ref().to_vec()))
        }

        /// Serializa el valor a bytes según RESP2.
        pub fn encode(&self) -> Vec<u8> {
            let mut out = Vec::new();
            self.encode_into(&mut out);
            out
        }

        fn encode_into(&self, out: &mut Vec<u8>) {
            match self {
                Value::Simple(s) => out.extend_from_slice(format!("+{s}\r\n").as_bytes()),
                Value::Error(s) => out.extend_from_slice(format!("-{s}\r\n").as_bytes()),
                Value::Int(i) => out.extend_from_slice(format!(":{i}\r\n").as_bytes()),
                Value::Bulk(None) => out.extend_from_slice(b"$-1\r\n"),
                Value::Bulk(Some(b)) => {
                    out.extend_from_slice(format!("${}\r\n", b.len()).as_bytes());
                    out.extend_from_slice(b);
                    out.extend_from_slice(b"\r\n");
                }
                Value::Array(None) => out.extend_from_slice(b"*-1\r\n"),
                Value::Array(Some(items)) => {
                    out.extend_from_slice(format!("*{}\r\n", items.len()).as_bytes());
                    for it in items {
                        it.encode_into(out);
                    }
                }
            }
        }
    }

    /// Resultado de intentar parsear: puede faltar información todavía.
    #[derive(Debug, PartialEq, Eq)]
    pub enum Parse {
        /// Se obtuvo un valor completo y se consumieron `usize` bytes.
        Done(Value, usize),
        /// El buffer no contiene todavía un valor completo.
        Incomplete,
    }

    /// Busca `\r\n` a partir de `start` y devuelve el índice del `\r`.
    fn find_crlf(buf: &[u8], start: usize) -> Option<usize> {
        (start..buf.len().saturating_sub(1)).find(|&i| buf[i] == b'\r' && buf[i + 1] == b'\n')
    }

    /// Parser incremental: recibe lo acumulado del socket y dice si hay un
    /// valor completo. Así un comando partido en varios `read()` funciona.
    pub fn parse(buf: &[u8]) -> Result<Parse, String> {
        parse_at(buf, 0)
    }

    fn parse_at(buf: &[u8], pos: usize) -> Result<Parse, String> {
        if pos >= buf.len() {
            return Ok(Parse::Incomplete);
        }
        let tag = buf[pos];
        // Línea de cabecera: desde después del tag hasta \r\n.
        let Some(end) = find_crlf(buf, pos + 1) else { return Ok(Parse::Incomplete) };
        let line = std::str::from_utf8(&buf[pos + 1..end]).map_err(|e| e.to_string())?;
        let after = end + 2;
        match tag {
            b'+' => Ok(Parse::Done(Value::Simple(line.into()), after - pos)),
            b'-' => Ok(Parse::Done(Value::Error(line.into()), after - pos)),
            b':' => {
                let n = line.parse::<i64>().map_err(|e| e.to_string())?;
                Ok(Parse::Done(Value::Int(n), after - pos))
            }
            b'$' => {
                let n: i64 = line.parse().map_err(|_| "longitud bulk inválida".to_string())?;
                if n < 0 {
                    return Ok(Parse::Done(Value::Bulk(None), after - pos));
                }
                let n = n as usize;
                // Necesitamos n bytes de datos + \r\n final.
                if buf.len() < after + n + 2 {
                    return Ok(Parse::Incomplete);
                }
                let data = buf[after..after + n].to_vec();
                Ok(Parse::Done(Value::Bulk(Some(data)), after + n + 2 - pos))
            }
            b'*' => {
                let n: i64 = line.parse().map_err(|_| "longitud array inválida".to_string())?;
                if n < 0 {
                    return Ok(Parse::Done(Value::Array(None), after - pos));
                }
                let mut items = Vec::with_capacity(n as usize);
                let mut cur = after;
                for _ in 0..n {
                    match parse_at(buf, cur)? {
                        Parse::Done(v, used) => {
                            items.push(v);
                            cur += used;
                        }
                        Parse::Incomplete => return Ok(Parse::Incomplete),
                    }
                }
                Ok(Parse::Done(Value::Array(Some(items)), cur - pos))
            }
            // Soporte "inline commands" (ej. escribir `PING` por telnet).
            _ => {
                let parts = std::str::from_utf8(&buf[pos..end])
                    .map_err(|e| e.to_string())?
                    .split_whitespace()
                    .map(Value::bulk)
                    .collect();
                Ok(Parse::Done(Value::Array(Some(parts)), after - pos))
            }
        }
    }
}

pub mod store {
    //! Almacén en memoria con expiración perezosa (lazy expiry).
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    /// Entrada del diccionario: valor + instante de expiración opcional.
    #[derive(Debug, Clone)]
    pub struct Entry {
        pub value: Vec<u8>,
        pub expires_at: Option<Instant>,
    }

    /// El diccionario principal. No es thread-safe por sí solo: el servidor
    /// lo envuelve en `Arc<Mutex<_>>`.
    #[derive(Debug, Default)]
    pub struct Store {
        map: HashMap<String, Entry>,
    }

    impl Store {
        pub fn new() -> Self {
            Self::default()
        }

        /// Elimina la clave si ya expiró. Redis real combina esto con un
        /// barrido activo periódico; aquí alcanza con la expiración perezosa.
        fn purge(&mut self, key: &str) {
            if let Some(e) = self.map.get(key) {
                if e.expires_at.is_some_and(|t| Instant::now() >= t) {
                    self.map.remove(key);
                }
            }
        }

        pub fn set(&mut self, key: &str, value: Vec<u8>, ttl: Option<Duration>) {
            let expires_at = ttl.map(|d| Instant::now() + d);
            self.map.insert(key.to_string(), Entry { value, expires_at });
        }

        pub fn get(&mut self, key: &str) -> Option<Vec<u8>> {
            self.purge(key);
            self.map.get(key).map(|e| e.value.clone())
        }

        pub fn del(&mut self, key: &str) -> bool {
            self.purge(key);
            self.map.remove(key).is_some()
        }

        pub fn exists(&mut self, key: &str) -> bool {
            self.purge(key);
            self.map.contains_key(key)
        }

        /// INCRBY: interpreta el valor como entero decimal.
        pub fn incr_by(&mut self, key: &str, delta: i64) -> Result<i64, String> {
            self.purge(key);
            let cur = match self.map.get(key) {
                None => 0,
                Some(e) => std::str::from_utf8(&e.value)
                    .ok()
                    .and_then(|s| s.parse::<i64>().ok())
                    .ok_or("ERR value is not an integer or out of range")?,
            };
            let n = cur.checked_add(delta).ok_or("ERR increment or decrement would overflow")?;
            let exp = self.map.get(key).and_then(|e| e.expires_at);
            self.map.insert(key.into(), Entry { value: n.to_string().into_bytes(), expires_at: exp });
            Ok(n)
        }

        /// Fija un TTL a una clave existente. Devuelve `false` si no existe.
        pub fn expire(&mut self, key: &str, ttl: Duration) -> bool {
            self.purge(key);
            match self.map.get_mut(key) {
                Some(e) => {
                    e.expires_at = Some(Instant::now() + ttl);
                    true
                }
                None => false,
            }
        }

        /// TTL en segundos: -2 si no existe, -1 si no expira (como Redis).
        pub fn ttl(&mut self, key: &str) -> i64 {
            self.purge(key);
            match self.map.get(key) {
                None => -2,
                Some(Entry { expires_at: None, .. }) => -1,
                Some(Entry { expires_at: Some(t), .. }) => {
                    t.saturating_duration_since(Instant::now()).as_secs() as i64
                }
            }
        }

        /// Lista de claves que coinciden con un patrón glob simple (`*`, `?`).
        pub fn keys(&mut self, pattern: &str) -> Vec<String> {
            let all: Vec<String> = self.map.keys().cloned().collect();
            for k in &all {
                self.purge(k);
            }
            let mut v: Vec<String> =
                self.map.keys().filter(|k| glob(pattern.as_bytes(), k.as_bytes())).cloned().collect();
            v.sort();
            v
        }

        pub fn len(&self) -> usize {
            self.map.len()
        }

        pub fn is_empty(&self) -> bool {
            self.map.is_empty()
        }

        pub fn clear(&mut self) {
            self.map.clear();
        }

        /// Itera todas las entradas vivas (usado por la persistencia).
        pub fn entries(&self) -> impl Iterator<Item = (&String, &Entry)> {
            let now = Instant::now();
            self.map.iter().filter(move |(_, e)| e.expires_at.map_or(true, |t| t > now))
        }
    }

    /// Matching glob recursivo minimalista: `*` = cualquier secuencia, `?` = un byte.
    pub fn glob(p: &[u8], s: &[u8]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(b'*'), _) => glob(&p[1..], s) || (!s.is_empty() && glob(p, &s[1..])),
            (Some(b'?'), Some(_)) => glob(&p[1..], &s[1..]),
            (Some(a), Some(b)) if a == b => glob(&p[1..], &s[1..]),
            _ => false,
        }
    }
}

pub mod persist {
    //! Snapshot a disco. Formato propio, simple y legible:
    //! el archivo es un arreglo RESP de tripletas `[clave, valor, ttl_ms]`
    //! (ttl_ms = -1 si no expira). Reusar RESP evita escribir otro parser.
    use crate::resp::{self, Value};
    use crate::store::Store;
    use std::io;
    use std::path::Path;
    use std::time::{Duration, Instant};

    pub fn save(store: &Store, path: &Path) -> io::Result<()> {
        let now = Instant::now();
        let mut items = Vec::new();
        for (k, e) in store.entries() {
            let ttl = e.expires_at.map_or(-1, |t| t.saturating_duration_since(now).as_millis() as i64);
            items.push(Value::bulk(k));
            items.push(Value::Bulk(Some(e.value.clone())));
            items.push(Value::Int(ttl));
        }
        // Escritura atómica: primero a un temporal y luego `rename`.
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, Value::Array(Some(items)).encode())?;
        std::fs::rename(tmp, path)
    }

    pub fn load(path: &Path) -> io::Result<Store> {
        let mut store = Store::new();
        if !path.exists() {
            return Ok(store);
        }
        let bytes = std::fs::read(path)?;
        let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_string());
        let v = match resp::parse(&bytes).map_err(|e| bad(&e))? {
            resp::Parse::Done(v, _) => v,
            resp::Parse::Incomplete => return Err(bad("snapshot truncado")),
        };
        let Value::Array(Some(items)) = v else { return Err(bad("formato inválido")) };
        for chunk in items.chunks(3) {
            if let [Value::Bulk(Some(k)), Value::Bulk(Some(val)), Value::Int(ttl)] = chunk {
                let ttl = (*ttl >= 0).then(|| Duration::from_millis(*ttl as u64));
                store.set(&String::from_utf8_lossy(k), val.clone(), ttl);
            }
        }
        Ok(store)
    }
}

pub mod server {
    //! Servidor TCP: un hilo por cliente, estado compartido con `Mutex`.
    use crate::persist;
    use crate::resp::{self, Parse, Value};
    use crate::store::Store;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// Estado compartido entre hilos.
    #[derive(Clone)]
    pub struct Shared {
        pub store: Arc<Mutex<Store>>,
        /// Ruta del snapshot (si hay persistencia habilitada).
        pub snapshot: Option<PathBuf>,
    }

    impl Shared {
        pub fn new(snapshot: Option<PathBuf>) -> std::io::Result<Self> {
            let store = match &snapshot {
                Some(p) => persist::load(p)?,
                None => Store::new(),
            };
            Ok(Self { store: Arc::new(Mutex::new(store)), snapshot })
        }
    }

    /// Convierte los argumentos RESP a `String` (los comandos son texto).
    fn args_of(v: Value) -> Result<Vec<Vec<u8>>, String> {
        match v {
            Value::Array(Some(items)) => items
                .into_iter()
                .map(|i| match i {
                    Value::Bulk(Some(b)) => Ok(b),
                    Value::Simple(s) => Ok(s.into_bytes()),
                    _ => Err("ERR argumento inválido".to_string()),
                })
                .collect(),
            _ => Err("ERR se esperaba un arreglo".into()),
        }
    }

    /// Ejecuta un comando ya parseado y devuelve la respuesta RESP.
    pub fn execute(shared: &Shared, cmd: Value) -> Value {
        let args = match args_of(cmd) {
            Ok(a) if !a.is_empty() => a,
            Ok(_) => return Value::Error("ERR comando vacío".into()),
            Err(e) => return Value::Error(e),
        };
        let name = String::from_utf8_lossy(&args[0]).to_uppercase();
        let s = |i: usize| String::from_utf8_lossy(&args[i]).to_string();
        let arity = |n: usize| args.len() == n;
        let wrong = || Value::Error(format!("ERR wrong number of arguments for '{}'", name.to_lowercase()));
        let mut st = shared.store.lock().unwrap();
        match name.as_str() {
            "PING" => match args.len() {
                1 => Value::Simple("PONG".into()),
                2 => Value::Bulk(Some(args[1].clone())),
                _ => wrong(),
            },
            "ECHO" if arity(2) => Value::Bulk(Some(args[1].clone())),
            "SET" if args.len() == 3 || args.len() == 5 => {
                // SET clave valor [EX segundos | PX milisegundos]
                let ttl = if args.len() == 5 {
                    let n: u64 = match s(4).parse() {
                        Ok(n) => n,
                        Err(_) => return Value::Error("ERR value is not an integer or out of range".into()),
                    };
                    match s(3).to_uppercase().as_str() {
                        "EX" => Some(Duration::from_secs(n)),
                        "PX" => Some(Duration::from_millis(n)),
                        _ => return Value::Error("ERR syntax error".into()),
                    }
                } else {
                    None
                };
                st.set(&s(1), args[2].clone(), ttl);
                Value::Simple("OK".into())
            }
            "GET" if arity(2) => Value::Bulk(st.get(&s(1))),
            "DEL" if args.len() >= 2 => Value::Int((1..args.len()).filter(|&i| st.del(&s(i))).count() as i64),
            "EXISTS" if args.len() >= 2 => Value::Int((1..args.len()).filter(|&i| st.exists(&s(i))).count() as i64),
            "INCR" if arity(2) => st.incr_by(&s(1), 1).map_or_else(Value::Error, Value::Int),
            "DECR" if arity(2) => st.incr_by(&s(1), -1).map_or_else(Value::Error, Value::Int),
            "INCRBY" if arity(3) => match s(2).parse() {
                Ok(d) => st.incr_by(&s(1), d).map_or_else(Value::Error, Value::Int),
                Err(_) => Value::Error("ERR value is not an integer or out of range".into()),
            },
            "EXPIRE" if arity(3) => match s(2).parse() {
                Ok(n) => Value::Int(st.expire(&s(1), Duration::from_secs(n)) as i64),
                Err(_) => Value::Error("ERR value is not an integer or out of range".into()),
            },
            "TTL" if arity(2) => Value::Int(st.ttl(&s(1))),
            "KEYS" if arity(2) => Value::Array(Some(st.keys(&s(1)).into_iter().map(Value::bulk).collect())),
            "DBSIZE" if arity(1) => Value::Int(st.len() as i64),
            "FLUSHALL" if arity(1) => {
                st.clear();
                Value::Simple("OK".into())
            }
            "SAVE" if arity(1) => match &shared.snapshot {
                Some(p) => match persist::save(&st, p) {
                    Ok(()) => Value::Simple("OK".into()),
                    Err(e) => Value::Error(format!("ERR {e}")),
                },
                None => Value::Error("ERR persistencia deshabilitada".into()),
            },
            "ECHO" | "SET" | "GET" | "DEL" | "EXISTS" | "INCR" | "DECR" | "INCRBY" | "EXPIRE"
            | "TTL" | "KEYS" | "DBSIZE" | "FLUSHALL" | "SAVE" => wrong(),
            _ => Value::Error(format!("ERR unknown command '{}'", name.to_lowercase())),
        }
    }

    /// Atiende una conexión: acumula bytes, parsea comandos y responde.
    /// Soporta *pipelining*: varios comandos en el mismo paquete.
    pub fn handle(shared: Shared, mut sock: TcpStream) -> std::io::Result<()> {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = sock.read(&mut chunk)?;
            if n == 0 {
                return Ok(()); // el cliente cerró
            }
            buf.extend_from_slice(&chunk[..n]);
            loop {
                match resp::parse(&buf) {
                    Ok(Parse::Done(v, used)) => {
                        buf.drain(..used);
                        let reply = execute(&shared, v);
                        sock.write_all(&reply.encode())?;
                    }
                    Ok(Parse::Incomplete) => break,
                    Err(e) => {
                        sock.write_all(&Value::Error(format!("ERR protocolo: {e}")).encode())?;
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Acepta conexiones para siempre (o hasta error del listener).
    pub fn serve(listener: TcpListener, shared: Shared) {
        for s in listener.incoming().flatten() {
            let sh = shared.clone();
            std::thread::spawn(move || {
                let _ = handle(sh, s);
            });
        }
    }
}

pub mod client {
    //! Cliente bloqueante mínimo.
    use crate::resp::{self, Parse, Value};
    use std::io::{Read, Write};
    use std::net::TcpStream;

    pub struct Client {
        sock: TcpStream,
        buf: Vec<u8>,
    }

    impl Client {
        pub fn connect(addr: &str) -> std::io::Result<Self> {
            Ok(Self { sock: TcpStream::connect(addr)?, buf: Vec::new() })
        }

        /// Envía un comando (lista de argumentos) y espera la respuesta.
        pub fn cmd(&mut self, args: &[&str]) -> std::io::Result<Value> {
            let v = Value::Array(Some(args.iter().map(Value::bulk).collect()));
            self.sock.write_all(&v.encode())?;
            let mut chunk = [0u8; 4096];
            loop {
                if let Parse::Done(v, used) = resp::parse(&self.buf)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
                {
                    self.buf.drain(..used);
                    return Ok(v);
                }
                let n = self.sock.read(&mut chunk)?;
                if n == 0 {
                    return Err(std::io::ErrorKind::UnexpectedEof.into());
                }
                self.buf.extend_from_slice(&chunk[..n]);
            }
        }
    }

    /// Formatea una respuesta como lo hace `redis-cli`.
    pub fn pretty(v: &Value) -> String {
        match v {
            Value::Simple(s) => s.clone(),
            Value::Error(e) => format!("(error) {e}"),
            Value::Int(i) => format!("(integer) {i}"),
            Value::Bulk(None) | Value::Array(None) => "(nil)".into(),
            Value::Bulk(Some(b)) => format!("\"{}\"", String::from_utf8_lossy(b)),
            Value::Array(Some(items)) if items.is_empty() => "(empty array)".into(),
            Value::Array(Some(items)) => items
                .iter()
                .enumerate()
                .map(|(i, x)| format!("{}) {}", i + 1, pretty(x)))
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::resp::{parse, Parse, Value};
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn resp_roundtrip() {
        let v = Value::Array(Some(vec![
            Value::bulk("SET"),
            Value::Int(-7),
            Value::Simple("OK".into()),
            Value::Bulk(None),
            Value::Error("ERR x".into()),
        ]));
        let enc = v.encode();
        assert_eq!(parse(&enc).unwrap(), Parse::Done(v, enc.len()));
    }

    #[test]
    fn resp_incompleto_y_inline() {
        assert_eq!(parse(b"*2\r\n$3\r\nGET\r\n$1\r\n").unwrap(), Parse::Incomplete);
        match parse(b"PING hola\r\n").unwrap() {
            Parse::Done(Value::Array(Some(v)), 11) => assert_eq!(v.len(), 2),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn glob_basico() {
        assert!(store::glob(b"us*", b"usuario:1"));
        assert!(store::glob(b"k?y", b"key"));
        assert!(!store::glob(b"a*b", b"acd"));
    }

    #[test]
    fn ttl_expira() {
        let mut s = store::Store::new();
        s.set("k", b"v".to_vec(), Some(std::time::Duration::from_millis(30)));
        assert_eq!(s.get("k"), Some(b"v".to_vec()));
        std::thread::sleep(std::time::Duration::from_millis(60));
        assert_eq!(s.get("k"), None);
        assert_eq!(s.ttl("k"), -2);
    }

    #[test]
    fn persistencia_snapshot() {
        let dir = std::env::temp_dir().join(format!("mredis-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("dump.resp");
        let mut s = store::Store::new();
        s.set("a", b"1".to_vec(), None);
        s.set("b", b"dos".to_vec(), Some(std::time::Duration::from_secs(100)));
        persist::save(&s, &p).unwrap();
        let mut l = persist::load(&p).unwrap();
        assert_eq!(l.get("a"), Some(b"1".to_vec()));
        assert!(l.ttl("b") > 90);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn servidor_end_to_end() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let sh = server::Shared::new(None).unwrap();
        std::thread::spawn(move || server::serve(l, sh));
        let mut c = client::Client::connect(&addr).unwrap();
        assert_eq!(c.cmd(&["PING"]).unwrap(), Value::Simple("PONG".into()));
        assert_eq!(c.cmd(&["SET", "x", "10"]).unwrap(), Value::Simple("OK".into()));
        assert_eq!(c.cmd(&["INCRBY", "x", "5"]).unwrap(), Value::Int(15));
        assert_eq!(c.cmd(&["GET", "x"]).unwrap(), Value::bulk("15"));
        assert_eq!(c.cmd(&["DEL", "x", "nope"]).unwrap(), Value::Int(1));
        assert_eq!(c.cmd(&["GET", "x"]).unwrap(), Value::Bulk(None));
        assert!(matches!(c.cmd(&["NOEXISTE"]).unwrap(), Value::Error(_)));
        assert!(matches!(c.cmd(&["GET"]).unwrap(), Value::Error(_)));
    }

    #[test]
    fn pipelining() {
        let sh = server::Shared::new(None).unwrap();
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || server::serve(l, sh));
        use std::io::{Read, Write};
        let mut s = std::net::TcpStream::connect(addr).unwrap();
        s.write_all(b"*1\r\n$4\r\nPING\r\n*2\r\n$4\r\nECHO\r\n$2\r\nhi\r\n").unwrap();
        let mut got = Vec::new();
        let mut b = [0u8; 64];
        while got.len() < 15 {
            let n = s.read(&mut b).unwrap();
            got.extend_from_slice(&b[..n]);
        }
        assert_eq!(&got, b"+PONG\r\n$2\r\nhi\r\n");
    }
}
