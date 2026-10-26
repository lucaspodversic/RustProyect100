//! # FUSE básico (userspace simulado)
//!
//! Modelamos la API que un filesystem FUSE real expondría (`lookup`,
//! `getattr`, `readdir`, `read`, `write`, `create`, `unlink`) sobre un
//! árbol en memoria. El path real (`fuse` crate + kernel module) se
//! documenta tras el feature `linux`.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType { Reg, Dir }

#[derive(Debug, Clone)]
pub struct Attr {
    pub ino: u64,
    pub size: u64,
    pub kind: FileType,
    pub mtime: u64,
}

#[derive(Debug)]
enum Node {
    File { attr: Attr, data: Vec<u8> },
    Dir { attr: Attr, children: BTreeMap<String, u64> },
}

impl Node {
    fn attr(&self) -> &Attr {
        match self {
            Node::File { attr, .. } | Node::Dir { attr, .. } => attr,
        }
    }
    fn attr_mut(&mut self) -> &mut Attr {
        match self {
            Node::File { attr, .. } | Node::Dir { attr, .. } => attr,
        }
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// Filesystem en memoria con inodos.
#[derive(Debug)]
pub struct MemFs {
    nodes: BTreeMap<u64, Node>,
    next_ino: u64,
}

impl MemFs {
    pub fn new() -> Self {
        let mut nodes = BTreeMap::new();
        nodes.insert(1, Node::Dir {
            attr: Attr { ino: 1, size: 0, kind: FileType::Dir, mtime: now() },
            children: BTreeMap::new(),
        });
        Self { nodes, next_ino: 2 }
    }

    fn alloc_ino(&mut self) -> u64 {
        let i = self.next_ino;
        self.next_ino += 1;
        i
    }

    pub fn getattr(&self, ino: u64) -> Result<Attr, String> {
        self.nodes.get(&ino).map(|n| n.attr().clone()).ok_or("ENOENT".into())
    }

    pub fn lookup(&self, parent: u64, name: &str) -> Result<Attr, String> {
        match self.nodes.get(&parent) {
            Some(Node::Dir { children, .. }) => {
                let ino = children.get(name).ok_or("ENOENT")?;
                self.getattr(*ino)
            }
            _ => Err("ENOTDIR".into()),
        }
    }

    pub fn readdir(&self, ino: u64) -> Result<Vec<(String, Attr)>, String> {
        match self.nodes.get(&ino) {
            Some(Node::Dir { children, .. }) => {
                let mut v = Vec::new();
                for (name, cino) in children {
                    v.push((name.clone(), self.getattr(*cino)?));
                }
                Ok(v)
            }
            _ => Err("ENOTDIR".into()),
        }
    }

    pub fn create(&mut self, parent: u64, name: &str) -> Result<Attr, String> {
        if !matches!(self.nodes.get(&parent), Some(Node::Dir { .. })) {
            return Err("ENOTDIR".into());
        }
        if let Node::Dir { children, .. } = &self.nodes[&parent] {
            if children.contains_key(name) {
                return Err("EEXIST".into());
            }
        }
        let ino = self.alloc_ino();
        let attr = Attr { ino, size: 0, kind: FileType::Reg, mtime: now() };
        self.nodes.insert(ino, Node::File { attr: attr.clone(), data: Vec::new() });
        if let Node::Dir { children, attr: pa, .. } = self.nodes.get_mut(&parent).unwrap() {
            children.insert(name.into(), ino);
            pa.mtime = now();
        }
        Ok(attr)
    }

    pub fn mkdir(&mut self, parent: u64, name: &str) -> Result<Attr, String> {
        if !matches!(self.nodes.get(&parent), Some(Node::Dir { .. })) {
            return Err("ENOTDIR".into());
        }
        let ino = self.alloc_ino();
        let attr = Attr { ino, size: 0, kind: FileType::Dir, mtime: now() };
        self.nodes.insert(ino, Node::Dir { attr: attr.clone(), children: BTreeMap::new() });
        if let Node::Dir { children, attr: pa, .. } = self.nodes.get_mut(&parent).unwrap() {
            children.insert(name.into(), ino);
            pa.mtime = now();
        }
        Ok(attr)
    }

    pub fn write(&mut self, ino: u64, offset: u64, data: &[u8]) -> Result<u32, String> {
        match self.nodes.get_mut(&ino) {
            Some(Node::File { attr, data: buf }) => {
                let off = offset as usize;
                if buf.len() < off + data.len() {
                    buf.resize(off + data.len(), 0);
                }
                buf[off..off + data.len()].copy_from_slice(data);
                attr.size = buf.len() as u64;
                attr.mtime = now();
                Ok(data.len() as u32)
            }
            Some(_) => Err("EISDIR".into()),
            None => Err("ENOENT".into()),
        }
    }

    pub fn read(&self, ino: u64, offset: u64, size: u32) -> Result<Vec<u8>, String> {
        match self.nodes.get(&ino) {
            Some(Node::File { data, .. }) => {
                let off = offset as usize;
                if off >= data.len() {
                    return Ok(vec![]);
                }
                let end = (off + size as usize).min(data.len());
                Ok(data[off..end].to_vec())
            }
            Some(_) => Err("EISDIR".into()),
            None => Err("ENOENT".into()),
        }
    }

    pub fn unlink(&mut self, parent: u64, name: &str) -> Result<(), String> {
        let ino = match self.nodes.get(&parent) {
            Some(Node::Dir { children, .. }) => *children.get(name).ok_or("ENOENT")?,
            _ => return Err("ENOTDIR".into()),
        };
        if matches!(self.nodes.get(&ino), Some(Node::Dir { children, .. }) if !children.is_empty()) {
            return Err("ENOTEMPTY".into());
        }
        self.nodes.remove(&ino);
        if let Node::Dir { children, attr, .. } = self.nodes.get_mut(&parent).unwrap() {
            children.remove(name);
            attr.mtime = now();
        }
        Ok(())
    }
}

impl Default for MemFs { fn default() -> Self { Self::new() } }

#[cfg(feature = "linux")]
pub mod linux_docs {
    //! Path real: crate `fuser` + `mount -t fuse`. El kernel habla FUSE
    //! por `/dev/fuse`; el proceso userspace responde las mismas ops que
    //! `MemFs` implementa. Requiere permisos de mount.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crear_escribir_leer() {
        let mut fs = MemFs::new();
        let a = fs.create(1, "hola.txt").unwrap();
        assert_eq!(a.kind, FileType::Reg);
        assert_eq!(fs.write(a.ino, 0, b"hola mundo").unwrap(), 10);
        assert_eq!(fs.read(a.ino, 0, 4).unwrap(), b"hola");
        assert_eq!(fs.lookup(1, "hola.txt").unwrap().size, 10);
        let entries = fs.readdir(1).unwrap();
        assert_eq!(entries.len(), 1);
    }
    #[test]
    fn mkdir_unlink() {
        let mut fs = MemFs::new();
        let d = fs.mkdir(1, "sub").unwrap();
        fs.create(d.ino, "a").unwrap();
        assert!(fs.unlink(1, "sub").unwrap_err().contains("ENOTEMPTY"));
        fs.unlink(d.ino, "a").unwrap();
        fs.unlink(1, "sub").unwrap();
        assert!(fs.lookup(1, "sub").is_err());
    }
}
