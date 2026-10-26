//! # Mini Kafka
//!
//! Cola de mensajes con **topics particionados**, **logs append-only**
//! persistidos en disco, **grupos de consumidores** con asignación de
//! particiones (estrategia *range*) y **offsets comprometidos** por grupo.
//!
//! Capas:
//! - [`log`]: log de una partición (memoria + archivo de segmento opcional).
//! - [`broker`]: topics, particionado por clave, grupos y rebalanceo.
//! - [`net`]: protocolo de texto simple sobre TCP + cliente.

pub mod log {
    //! Un log append-only. Cada registro recibe un *offset* monótono.
    //! Formato en disco por registro: `[u32 len_key][key][u32 len_val][val]`
    //! (little endian). Al abrir se re-lee el archivo para reconstruir el índice.
    use std::fs::{File, OpenOptions};
    use std::io::{self, Read, Write};
    use std::path::{Path, PathBuf};

    /// Un mensaje almacenado.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Record {
        pub offset: u64,
        pub key: Vec<u8>,
        pub value: Vec<u8>,
    }

    /// Log de una partición.
    #[derive(Debug)]
    pub struct PartitionLog {
        records: Vec<Record>,
        file: Option<File>,
        path: Option<PathBuf>,
    }

    impl PartitionLog {
        /// Log sólo en memoria (útil para tests).
        pub fn in_memory() -> Self {
            Self { records: Vec::new(), file: None, path: None }
        }

        /// Abre (o crea) un log persistente y recupera los registros previos.
        pub fn open(path: &Path) -> io::Result<Self> {
            let mut records = Vec::new();
            if path.exists() {
                let mut data = Vec::new();
                File::open(path)?.read_to_end(&mut data)?;
                let mut i = 0usize;
                // Leemos registros completos; si el último quedó truncado por
                // un corte de energía, lo descartamos (como hace Kafka).
                let rd = |d: &[u8], i: usize| -> Option<u32> {
                    d.get(i..i + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                };
                loop {
                    let Some(kl) = rd(&data, i) else { break };
                    let ks = i + 4;
                    let Some(vl) = rd(&data, ks + kl as usize) else { break };
                    let vs = ks + kl as usize + 4;
                    if data.len() < vs + vl as usize {
                        break;
                    }
                    records.push(Record {
                        offset: records.len() as u64,
                        key: data[ks..ks + kl as usize].to_vec(),
                        value: data[vs..vs + vl as usize].to_vec(),
                    });
                    i = vs + vl as usize;
                }
            }
            let file = OpenOptions::new().create(true).append(true).open(path)?;
            Ok(Self { records, file: Some(file), path: Some(path.to_path_buf()) })
        }

        /// Agrega un registro y devuelve su offset.
        pub fn append(&mut self, key: &[u8], value: &[u8]) -> io::Result<u64> {
            if let Some(f) = self.file.as_mut() {
                let mut buf = Vec::with_capacity(8 + key.len() + value.len());
                buf.extend_from_slice(&(key.len() as u32).to_le_bytes());
                buf.extend_from_slice(key);
                buf.extend_from_slice(&(value.len() as u32).to_le_bytes());
                buf.extend_from_slice(value);
                f.write_all(&buf)?;
            }
            let offset = self.records.len() as u64;
            self.records.push(Record { offset, key: key.to_vec(), value: value.to_vec() });
            Ok(offset)
        }

        /// Lee hasta `max` registros desde `offset` inclusive.
        pub fn read(&self, offset: u64, max: usize) -> Vec<Record> {
            self.records.iter().skip(offset as usize).take(max).cloned().collect()
        }

        /// Offset del próximo registro (= cantidad de registros).
        pub fn end_offset(&self) -> u64 {
            self.records.len() as u64
        }

        pub fn path(&self) -> Option<&Path> {
            self.path.as_deref()
        }
    }
}

pub mod broker {
    //! El broker: dueño de los topics y de los grupos de consumidores.
    use crate::log::{PartitionLog, Record};
    use std::collections::{BTreeMap, BTreeSet, HashMap};
    use std::io;
    use std::path::PathBuf;

    /// Hash FNV-1a de 64 bits: determinista entre ejecuciones (a diferencia
    /// del `DefaultHasher`), necesario para que una clave caiga siempre en la
    /// misma partición y así conservar el orden por clave.
    pub fn fnv1a(data: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in data {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    /// Estado de un grupo de consumidores.
    #[derive(Debug, Default)]
    pub struct Group {
        /// Miembros activos (ordenados para que la asignación sea determinista).
        pub members: BTreeSet<String>,
        /// Offsets comprometidos: (topic, partición) -> próximo offset a leer.
        pub committed: HashMap<(String, u32), u64>,
        /// "Generación": se incrementa en cada rebalanceo.
        pub generation: u64,
    }

    #[derive(Debug, Default)]
    pub struct Broker {
        topics: BTreeMap<String, Vec<PartitionLog>>,
        groups: HashMap<String, Group>,
        data_dir: Option<PathBuf>,
        /// Contador round-robin para mensajes sin clave.
        rr: u64,
    }

    impl Broker {
        pub fn in_memory() -> Self {
            Self::default()
        }

        /// Broker persistente: cada partición es `dir/<topic>-<n>.log`.
        pub fn with_dir(dir: PathBuf) -> io::Result<Self> {
            std::fs::create_dir_all(&dir)?;
            let mut b = Self { data_dir: Some(dir.clone()), ..Self::default() };
            // Recuperar topics existentes a partir de los nombres de archivo.
            let mut found: BTreeMap<String, u32> = BTreeMap::new();
            for e in std::fs::read_dir(&dir)? {
                let name = e?.file_name().to_string_lossy().to_string();
                if let Some(stem) = name.strip_suffix(".log") {
                    if let Some((t, p)) = stem.rsplit_once('-') {
                        if let Ok(p) = p.parse::<u32>() {
                            let m = found.entry(t.to_string()).or_default();
                            *m = (*m).max(p + 1);
                        }
                    }
                }
            }
            for (t, n) in found {
                b.create_topic(&t, n)?;
            }
            Ok(b)
        }

        /// Crea un topic con `n` particiones (idempotente si ya existe).
        pub fn create_topic(&mut self, name: &str, n: u32) -> io::Result<()> {
            if self.topics.contains_key(name) {
                return Ok(());
            }
            if name.is_empty() || name.contains(['-', '/', ' ']) || n == 0 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "nombre/particiones inválidos"));
            }
            let mut parts = Vec::new();
            for p in 0..n {
                parts.push(match &self.data_dir {
                    Some(d) => PartitionLog::open(&d.join(format!("{name}-{p}.log")))?,
                    None => PartitionLog::in_memory(),
                });
            }
            self.topics.insert(name.to_string(), parts);
            Ok(())
        }

        pub fn partitions(&self, topic: &str) -> Option<u32> {
            self.topics.get(topic).map(|p| p.len() as u32)
        }

        /// Produce un mensaje. Con clave: partición = hash(clave) % n.
        /// Sin clave: round-robin. Devuelve (partición, offset).
        pub fn produce(&mut self, topic: &str, key: &[u8], value: &[u8]) -> io::Result<(u32, u64)> {
            let rr = self.rr;
            let parts = self
                .topics
                .get_mut(topic)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "topic inexistente"))?;
            let n = parts.len() as u64;
            let p = if key.is_empty() { rr % n } else { fnv1a(key) % n } as u32;
            self.rr += 1;
            let off = parts[p as usize].append(key, value)?;
            Ok((p, off))
        }

        pub fn fetch(&self, topic: &str, part: u32, offset: u64, max: usize) -> Option<Vec<Record>> {
            self.topics.get(topic)?.get(part as usize).map(|l| l.read(offset, max))
        }

        pub fn end_offset(&self, topic: &str, part: u32) -> Option<u64> {
            self.topics.get(topic)?.get(part as usize).map(|l| l.end_offset())
        }

        /// Un miembro se une a un grupo: dispara rebalanceo (nueva generación).
        pub fn join(&mut self, group: &str, member: &str) -> u64 {
            let g = self.groups.entry(group.into()).or_default();
            if g.members.insert(member.into()) {
                g.generation += 1;
            }
            g.generation
        }

        /// Un miembro abandona el grupo (o "muere"): también rebalancea.
        pub fn leave(&mut self, group: &str, member: &str) -> u64 {
            let g = self.groups.entry(group.into()).or_default();
            if g.members.remove(member) {
                g.generation += 1;
            }
            g.generation
        }

        /// Asignación **range**: las particiones se reparten en bloques
        /// contiguos; los primeros miembros reciben una extra si no divide justo.
        /// Cada partición la consume exactamente un miembro del grupo.
        pub fn assignment(&self, group: &str, topic: &str, member: &str) -> Vec<u32> {
            let (Some(g), Some(n)) = (self.groups.get(group), self.partitions(topic)) else {
                return vec![];
            };
            let members: Vec<&String> = g.members.iter().collect();
            let Some(idx) = members.iter().position(|m| *m == member) else { return vec![] };
            let m = members.len() as u32;
            let (base, extra) = (n / m, n % m);
            let i = idx as u32;
            let start = i * base + i.min(extra);
            let len = base + u32::from(i < extra);
            (start..start + len).collect()
        }

        pub fn commit(&mut self, group: &str, topic: &str, part: u32, next_offset: u64) {
            self.groups.entry(group.into()).or_default().committed.insert((topic.into(), part), next_offset);
        }

        pub fn committed(&self, group: &str, topic: &str, part: u32) -> u64 {
            self.groups
                .get(group)
                .and_then(|g| g.committed.get(&(topic.to_string(), part)).copied())
                .unwrap_or(0)
        }

        /// "Lag" del grupo: mensajes pendientes sumando todas las particiones.
        pub fn lag(&self, group: &str, topic: &str) -> u64 {
            (0..self.partitions(topic).unwrap_or(0))
                .map(|p| self.end_offset(topic, p).unwrap_or(0) - self.committed(group, topic, p))
                .sum()
        }

        /// Poll de conveniencia: lee de las particiones asignadas a partir
        /// del offset comprometido y comprometa automáticamente (auto-commit).
        pub fn poll(&mut self, group: &str, topic: &str, member: &str, max: usize) -> Vec<(u32, Record)> {
            let mut out = Vec::new();
            for p in self.assignment(group, topic, member) {
                let from = self.committed(group, topic, p);
                let recs = self.fetch(topic, p, from, max - out.len()).unwrap_or_default();
                if let Some(last) = recs.last() {
                    self.commit(group, topic, p, last.offset + 1);
                }
                out.extend(recs.into_iter().map(|r| (p, r)));
                if out.len() >= max {
                    break;
                }
            }
            out
        }
    }
}

pub mod net {
    //! Protocolo de texto línea por línea (fácil de probar con `nc`):
    //! ```text
    //! CREATE <topic> <n>                -> OK
    //! PRODUCE <topic> <key|-> <valor..> -> OK <part> <offset>
    //! JOIN <grupo> <miembro>            -> OK <generación>
    //! LEAVE <grupo> <miembro>           -> OK <generación>
    //! POLL <grupo> <topic> <miembro> <max> -> N líneas "MSG p off key valor" + "END"
    //! LAG <grupo> <topic>               -> OK <n>
    //! ```
    use crate::broker::Broker;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    pub fn handle_line(b: &Mutex<Broker>, line: &str) -> String {
        let p: Vec<&str> = line.split_whitespace().collect();
        let mut b = b.lock().unwrap();
        let err = |e: &dyn std::fmt::Display| format!("ERR {e}\n");
        match p.as_slice() {
            ["CREATE", t, n] => match n.parse() {
                Ok(n) => b.create_topic(t, n).map_or_else(|e| err(&e), |_| "OK\n".into()),
                Err(e) => err(&e),
            },
            ["PRODUCE", t, k, rest @ ..] if !rest.is_empty() => {
                let key = if *k == "-" { "" } else { k };
                match b.produce(t, key.as_bytes(), rest.join(" ").as_bytes()) {
                    Ok((p, o)) => format!("OK {p} {o}\n"),
                    Err(e) => err(&e),
                }
            }
            ["JOIN", g, m] => format!("OK {}\n", b.join(g, m)),
            ["LEAVE", g, m] => format!("OK {}\n", b.leave(g, m)),
            ["POLL", g, t, m, max] => {
                let max = max.parse().unwrap_or(10);
                let mut s = String::new();
                for (p, r) in b.poll(g, t, m, max) {
                    let k = String::from_utf8_lossy(&r.key);
                    let k = if k.is_empty() { "-".into() } else { k };
                    s += &format!("MSG {p} {} {k} {}\n", r.offset, String::from_utf8_lossy(&r.value));
                }
                s + "END\n"
            }
            ["LAG", g, t] => format!("OK {}\n", b.lag(g, t)),
            _ => "ERR comando desconocido\n".into(),
        }
    }

    pub fn serve(l: TcpListener, b: Arc<Mutex<Broker>>) {
        for s in l.incoming().flatten() {
            let b = b.clone();
            std::thread::spawn(move || {
                let mut w = s.try_clone().unwrap();
                for line in BufReader::new(s).lines().map_while(Result::ok) {
                    if w.write_all(handle_line(&b, &line).as_bytes()).is_err() {
                        break;
                    }
                }
            });
        }
    }

    /// Cliente: envía una línea y lee hasta "OK…/ERR…" o hasta "END".
    pub struct Client {
        r: BufReader<TcpStream>,
        w: TcpStream,
    }

    impl Client {
        pub fn connect(addr: &str) -> std::io::Result<Self> {
            let w = TcpStream::connect(addr)?;
            Ok(Self { r: BufReader::new(w.try_clone()?), w })
        }

        pub fn send(&mut self, line: &str) -> std::io::Result<Vec<String>> {
            self.w.write_all(format!("{line}\n").as_bytes())?;
            let mut out = Vec::new();
            loop {
                let mut s = String::new();
                if self.r.read_line(&mut s)? == 0 {
                    break;
                }
                let s = s.trim_end().to_string();
                let done = !s.starts_with("MSG");
                out.push(s);
                if done {
                    break;
                }
            }
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::broker::Broker;
    use super::*;

    #[test]
    fn misma_clave_misma_particion_y_orden() {
        let mut b = Broker::in_memory();
        b.create_topic("pedidos", 4).unwrap();
        let (p1, o1) = b.produce("pedidos", b"cliente-7", b"a").unwrap();
        let (p2, o2) = b.produce("pedidos", b"cliente-7", b"b").unwrap();
        assert_eq!(p1, p2);
        assert_eq!(o2, o1 + 1);
        let recs = b.fetch("pedidos", p1, 0, 10).unwrap();
        assert_eq!(recs.iter().map(|r| r.value.clone()).collect::<Vec<_>>(), vec![b"a".to_vec(), b"b".to_vec()]);
    }

    #[test]
    fn round_robin_sin_clave() {
        let mut b = Broker::in_memory();
        b.create_topic("t", 3).unwrap();
        let ps: Vec<u32> = (0..6).map(|_| b.produce("t", b"", b"x").unwrap().0).collect();
        assert_eq!(ps, vec![0, 1, 2, 0, 1, 2]);
    }

    #[test]
    fn asignacion_range_cubre_todo_sin_solapar() {
        let mut b = Broker::in_memory();
        b.create_topic("t", 7).unwrap();
        for m in ["c1", "c2", "c3"] {
            b.join("g", m);
        }
        let mut all: Vec<u32> = ["c1", "c2", "c3"].iter().flat_map(|m| b.assignment("g", "t", m)).collect();
        assert_eq!(b.assignment("g", "t", "c1"), vec![0, 1, 2]);
        all.sort();
        assert_eq!(all, (0..7).collect::<Vec<_>>());
        // Rebalanceo al irse un miembro.
        let gen = b.leave("g", "c2");
        assert_eq!(gen, 4);
        assert_eq!(b.assignment("g", "t", "c1").len() + b.assignment("g", "t", "c3").len(), 7);
    }

    #[test]
    fn grupos_independientes_y_lag() {
        let mut b = Broker::in_memory();
        b.create_topic("t", 2).unwrap();
        for i in 0..10 {
            b.produce("t", format!("k{i}").as_bytes(), b"v").unwrap();
        }
        b.join("a", "x");
        b.join("b", "y");
        assert_eq!(b.lag("a", "t"), 10);
        assert_eq!(b.poll("a", "t", "x", 100).len(), 10);
        assert_eq!(b.lag("a", "t"), 0);
        assert_eq!(b.lag("b", "t"), 10, "otro grupo no se ve afectado");
        assert!(b.poll("a", "t", "x", 100).is_empty(), "no se re-entregan mensajes");
    }

    #[test]
    fn persistencia_y_recuperacion() {
        let dir = std::env::temp_dir().join(format!("mkafka-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        {
            let mut b = Broker::with_dir(dir.clone()).unwrap();
            b.create_topic("logs", 2).unwrap();
            for i in 0..5 {
                b.produce("logs", b"", format!("m{i}").as_bytes()).unwrap();
            }
        }
        let b = Broker::with_dir(dir.clone()).unwrap();
        assert_eq!(b.partitions("logs"), Some(2));
        assert_eq!(b.end_offset("logs", 0).unwrap() + b.end_offset("logs", 1).unwrap(), 5);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn protocolo_tcp() {
        use std::sync::{Arc, Mutex};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        std::thread::spawn(move || net::serve(l, Arc::new(Mutex::new(Broker::in_memory()))));
        let mut c = net::Client::connect(&addr).unwrap();
        assert_eq!(c.send("CREATE ev 1").unwrap(), vec!["OK"]);
        assert_eq!(c.send("PRODUCE ev k1 hola mundo").unwrap(), vec!["OK 0 0"]);
        assert_eq!(c.send("JOIN g c").unwrap(), vec!["OK 1"]);
        assert_eq!(c.send("POLL g ev c 5").unwrap(), vec!["MSG 0 0 k1 hola mundo", "END"]);
        assert_eq!(c.send("LAG g ev").unwrap(), vec!["OK 0"]);
        assert!(c.send("XYZ").unwrap()[0].starts_with("ERR"));
    }
}
