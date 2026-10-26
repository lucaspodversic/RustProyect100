//! # Mini Git
//!
//! Control de versiones con **almacenamiento direccionado por contenido**
//! (como el object store de Git):
//!
//! - `Blob`  = contenido de un archivo.
//! - `Tree`  = directorio (nombre → hash).
//! - `Commit` = tree + padres + mensaje + autor.
//!
//! Los objetos se identifican por el SHA-1 del encabezado `tipo tamaño\0contenido`
//! (exactamente el mismo esquema que Git). Se guardan en `.minigit/objects/ab/cdef…`.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// SHA-1 mínimo (para no depender de crates externos). No es criptográficamente
/// auditado: alcanza para content-addressing educativo.
pub mod sha1 {
    pub fn hash(data: &[u8]) -> [u8; 20] {
        // Implementación compacta del SHA-1 (FIPS 180-4).
        fn f(t: u32, b: u32, c: u32, d: u32) -> u32 {
            match t {
                0..=19 => (b & c) | ((!b) & d),
                20..=39 | 60..=79 => b ^ c ^ d,
                _ => (b & c) | (b & d) | (c & d),
            }
        }
        fn k(t: u32) -> u32 {
            match t {
                0..=19 => 0x5A827999,
                20..=39 => 0x6ED9EBA1,
                40..=59 => 0x8F1BBCDC,
                _ => 0xCA62C1D6,
            }
        }
        let mut msg = data.to_vec();
        let bit_len = (data.len() as u64) * 8;
        msg.push(0x80);
        while (msg.len() % 64) != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&bit_len.to_be_bytes());
        let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
        for chunk in msg.chunks(64) {
            let mut w = [0u32; 80];
            for i in 0..16 {
                w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
            }
            for t in 16..80 {
                w[t] = (w[t - 3] ^ w[t - 8] ^ w[t - 14] ^ w[t - 16]).rotate_left(1);
            }
            let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
            for t in 0..80 {
                let temp = a
                    .rotate_left(5)
                    .wrapping_add(f(t as u32, b, c, d))
                    .wrapping_add(e)
                    .wrapping_add(k(t as u32))
                    .wrapping_add(w[t]);
                e = d;
                d = c;
                c = b.rotate_left(30);
                b = a;
                a = temp;
            }
            h[0] = h[0].wrapping_add(a);
            h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c);
            h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e);
        }
        let mut out = [0u8; 20];
        for (i, v) in h.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
        }
        out
    }

    pub fn hex(h: &[u8; 20]) -> String {
        h.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// Un identificador de objeto (20 bytes).
pub type Oid = [u8; 20];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Object {
    Blob(Vec<u8>),
    /// name → (mode, oid). Usamos BTreeMap para un encoding determinista.
    Tree(BTreeMap<String, (String, Oid)>),
    Commit {
        tree: Oid,
        parents: Vec<Oid>,
        author: String,
        message: String,
    },
}

impl Object {
    fn kind(&self) -> &'static str {
        match self {
            Object::Blob(_) => "blob",
            Object::Tree(_) => "tree",
            Object::Commit { .. } => "commit",
        }
    }

    /// Serializa al formato Git (sin zlib; Git real comprime).
    pub fn encode(&self) -> Vec<u8> {
        let body = match self {
            Object::Blob(b) => b.clone(),
            Object::Tree(entries) => {
                let mut body = Vec::new();
                for (name, (mode, oid)) in entries {
                    body.extend_from_slice(format!("{mode} {name}\0").as_bytes());
                    body.extend_from_slice(oid);
                }
                body
            }
            Object::Commit { tree, parents, author, message } => {
                let mut s = format!("tree {}\n", sha1::hex(tree));
                for p in parents {
                    s += &format!("parent {}\n", sha1::hex(p));
                }
                s += &format!("author {author}\n\n{message}\n");
                s.into_bytes()
            }
        };
        let mut out = format!("{} {}\0", self.kind(), body.len()).into_bytes();
        out.extend(body);
        out
    }

    pub fn oid(&self) -> Oid {
        sha1::hash(&self.encode())
    }

    pub fn decode(data: &[u8]) -> Result<Object, String> {
        let nul = data.iter().position(|&b| b == 0).ok_or("sin header")?;
        let header = std::str::from_utf8(&data[..nul]).map_err(|e| e.to_string())?;
        let (kind, size) = header.split_once(' ').ok_or("header inválido")?;
        let size: usize = size.parse().map_err(|_| "size")?;
        let body = &data[nul + 1..];
        if body.len() != size {
            return Err("tamaño inconsistente".into());
        }
        Ok(match kind {
            "blob" => Object::Blob(body.to_vec()),
            "tree" => {
                let mut entries = BTreeMap::new();
                let mut i = 0;
                while i < body.len() {
                    let sp = body[i..].iter().position(|&b| b == b' ').ok_or("mode")?;
                    let mode = std::str::from_utf8(&body[i..i + sp]).unwrap().to_string();
                    i += sp + 1;
                    let nul = body[i..].iter().position(|&b| b == 0).ok_or("name")?;
                    let name = std::str::from_utf8(&body[i..i + nul]).unwrap().to_string();
                    i += nul + 1;
                    let mut oid = [0u8; 20];
                    oid.copy_from_slice(&body[i..i + 20]);
                    i += 20;
                    entries.insert(name, (mode, oid));
                }
                Object::Tree(entries)
            }
            "commit" => {
                let text = std::str::from_utf8(body).map_err(|e| e.to_string())?;
                let (headers, message) = text.split_once("\n\n").ok_or("commit")?;
                let mut tree = None;
                let mut parents = Vec::new();
                let mut author = String::new();
                for line in headers.lines() {
                    if let Some(h) = line.strip_prefix("tree ") {
                        let mut o = [0u8; 20];
                        for (i, p) in h.as_bytes().chunks(2).enumerate() {
                            o[i] = u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap();
                        }
                        tree = Some(o);
                    } else if let Some(h) = line.strip_prefix("parent ") {
                        let mut o = [0u8; 20];
                        for (i, p) in h.as_bytes().chunks(2).enumerate() {
                            o[i] = u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap();
                        }
                        parents.push(o);
                    } else if let Some(a) = line.strip_prefix("author ") {
                        author = a.into();
                    }
                }
                Object::Commit {
                    tree: tree.ok_or("sin tree")?,
                    parents,
                    author,
                    message: message.trim_end_matches('\n').into(),
                }
            }
            _ => return Err("tipo desconocido".into()),
        })
    }
}

/// Un repositorio `.minigit`.
#[derive(Debug)]
pub struct Repo {
    pub root: PathBuf,
}

impl Repo {
    pub fn init(path: &Path) -> io::Result<Self> {
        let git = path.join(".minigit");
        fs::create_dir_all(git.join("objects"))?;
        fs::create_dir_all(git.join("refs/heads"))?;
        fs::write(git.join("HEAD"), b"ref: refs/heads/master\n")?;
        Ok(Self { root: path.to_path_buf() })
    }

    pub fn open(path: &Path) -> io::Result<Self> {
        let git = path.join(".minigit");
        if !git.join("objects").is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no es un repo"));
        }
        Ok(Self { root: path.to_path_buf() })
    }

    fn obj_path(&self, oid: &Oid) -> PathBuf {
        let h = sha1::hex(oid);
        self.root.join(".minigit/objects").join(&h[..2]).join(&h[2..])
    }

    pub fn store(&self, obj: &Object) -> io::Result<Oid> {
        let data = obj.encode();
        let oid = sha1::hash(&data);
        let p = self.obj_path(&oid);
        if !p.exists() {
            fs::create_dir_all(p.parent().unwrap())?;
            fs::write(p, data)?;
        }
        Ok(oid)
    }

    pub fn load(&self, oid: &Oid) -> io::Result<Object> {
        let data = fs::read(self.obj_path(oid))?;
        Object::decode(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Indexa un archivo del working tree como blob.
    pub fn hash_file(&self, rel: &str) -> io::Result<Oid> {
        let data = fs::read(self.root.join(rel))?;
        self.store(&Object::Blob(data))
    }

    /// Construye un tree a partir de (nombre, mode, oid).
    pub fn write_tree(&self, entries: BTreeMap<String, (String, Oid)>) -> io::Result<Oid> {
        self.store(&Object::Tree(entries))
    }

    pub fn commit(&self, tree: Oid, parents: Vec<Oid>, author: &str, msg: &str) -> io::Result<Oid> {
        let oid = self.store(&Object::Commit {
            tree,
            parents,
            author: author.into(),
            message: msg.into(),
        })?;
        // Avanza la rama actual.
        let head = fs::read_to_string(self.root.join(".minigit/HEAD"))?;
        if let Some(r) = head.strip_prefix("ref: ") {
            let r = r.trim();
            let p = self.root.join(".minigit").join(r);
            fs::create_dir_all(p.parent().unwrap())?;
            fs::write(p, format!("{}\n", sha1::hex(&oid)))?;
        }
        Ok(oid)
    }

    pub fn head(&self) -> io::Result<Option<Oid>> {
        let head = fs::read_to_string(self.root.join(".minigit/HEAD"))?;
        let target = if let Some(r) = head.strip_prefix("ref: ") {
            let p = self.root.join(".minigit").join(r.trim());
            if !p.exists() {
                return Ok(None);
            }
            fs::read_to_string(p)?
        } else {
            head
        };
        let h = target.trim();
        let mut oid = [0u8; 20];
        for (i, p) in h.as_bytes().chunks(2).enumerate() {
            oid[i] = u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap();
        }
        Ok(Some(oid))
    }

    /// Log lineal siguiendo el primer padre.
    pub fn log(&self, mut oid: Oid) -> io::Result<Vec<(Oid, String)>> {
        let mut out = Vec::new();
        loop {
            match self.load(&oid)? {
                Object::Commit { parents, message, .. } => {
                    out.push((oid, message));
                    match parents.first() {
                        Some(p) => oid = *p,
                        None => break,
                    }
                }
                _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "no es commit")),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn sha1_conocido() {
        // "abc" → a9993e364706816aba3e25717850c26c9cd0d89d
        assert_eq!(sha1::hex(&sha1::hash(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn content_addressed_dedup() {
        let dir = std::env::temp_dir().join(format!("mgit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let repo = Repo::init(&dir).unwrap();
        let a = repo.store(&Object::Blob(b"hola".to_vec())).unwrap();
        let b = repo.store(&Object::Blob(b"hola".to_vec())).unwrap();
        assert_eq!(a, b, "mismo contenido = mismo oid");
        match repo.load(&a).unwrap() {
            Object::Blob(v) => assert_eq!(v, b"hola"),
            _ => panic!(),
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn commit_y_log() {
        let dir = std::env::temp_dir().join(format!("mgit2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/main.rs"), b"fn main(){}").unwrap();
        let repo = Repo::init(&dir).unwrap();
        let blob = repo.hash_file("src/main.rs").unwrap();
        let mut entries = BTreeMap::new();
        entries.insert("main.rs".into(), ("100644".into(), blob));
        let tree_src = repo.write_tree(entries).unwrap();
        let mut root = BTreeMap::new();
        root.insert("src".into(), ("040000".into(), tree_src));
        let tree = repo.write_tree(root).unwrap();
        let c1 = repo.commit(tree, vec![], "Ada <ada@ex>", "inicial").unwrap();
        let c2 = repo.commit(tree, vec![c1], "Ada <ada@ex>", "sin cambios").unwrap();
        assert_eq!(repo.head().unwrap(), Some(c2));
        let log = repo.log(c2).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].1, "sin cambios");
        assert_eq!(log[1].0, c1);
        // Roundtrip tree
        match repo.load(&tree).unwrap() {
            Object::Tree(t) => assert!(t.contains_key("src")),
            _ => panic!(),
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
