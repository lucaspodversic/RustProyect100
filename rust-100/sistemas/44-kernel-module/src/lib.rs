//! # Módulo de kernel toy
//!
//! Simulamos el protocolo que un módulo hablaría con userspace vía un
//! dispositivo `/dev/toyctl`: comandos de texto `PING`, `ECHO`, `STATS`,
//! `SET name=value`. Los tests ejercitan esa API en userspace.
//! El esqueleto del módulo real (Rust-for-Linux) está tras `feature = "linux"`.

use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct ToyModule {
    pub params: Mutex<HashMap<String, String>>,
    pub hits: Mutex<u64>,
}

impl ToyModule {
    pub fn new() -> Self {
        Self::default()
    }

    /// Procesa un comando y devuelve la respuesta (como `read` tras `write`).
    pub fn handle(&self, cmd: &str) -> String {
        *self.hits.lock().unwrap() += 1;
        let cmd = cmd.trim();
        if cmd == "PING" {
            return "PONG\n".into();
        }
        if let Some(rest) = cmd.strip_prefix("ECHO ") {
            return format!("{rest}\n");
        }
        if cmd == "STATS" {
            let n = *self.hits.lock().unwrap();
            let p = self.params.lock().unwrap().len();
            return format!("hits={n} params={p}\n");
        }
        if let Some(rest) = cmd.strip_prefix("SET ") {
            if let Some((k, v)) = rest.split_once('=') {
                self.params.lock().unwrap().insert(k.into(), v.into());
                return "OK\n".into();
            }
            return "ERR sintaxis SET name=value\n".into();
        }
        if let Some(k) = cmd.strip_prefix("GET ") {
            return match self.params.lock().unwrap().get(k) {
                Some(v) => format!("{v}\n"),
                None => "ERR no existe\n".into(),
            };
        }
        "ERR desconocido\n".into()
    }
}

#[cfg(feature = "linux")]
pub mod linux_docs {
    //! Esqueleto Rust-for-Linux (no se compila aquí):
    //! ```ignore
    //! #[module]
    //! struct Toy;
    //! impl kernel::Module for Toy {
    //!     fn init(...) -> Result<Self> {
    //!         miscdev::Registration::new_pinned(vtable..., c_str!("toyctl"))?;
    //!         Ok(Toy)
    //!     }
    //! }
    //! ```
    //! Userspace: `echo PING > /dev/toyctl && cat /dev/toyctl`.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocolo() {
        let m = ToyModule::new();
        assert_eq!(m.handle("PING"), "PONG\n");
        assert_eq!(m.handle("ECHO hola"), "hola\n");
        assert_eq!(m.handle("SET foo=bar"), "OK\n");
        assert_eq!(m.handle("GET foo"), "bar\n");
        assert!(m.handle("STATS").contains("hits="));
        assert!(m.handle("XYZ").starts_with("ERR"));
    }
}
