//! Fuente de datos del explorador.
//!
//! Definimos el trait [`FuenteCadena`] para desacoplar la UI de dónde vienen
//! los bloques: en tests y en la demo usamos [`CadenaSimulada`], generada de
//! forma determinista; un backend RPC real (p. ej. JSON-RPC de una L2 tipo
//! SOON/Solana/EVM) solo tendría que implementar el mismo trait.

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tx {
    pub hash: String,
    pub de: String,
    pub para: String,
    pub monto: u64,
    pub comision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bloque {
    pub altura: u64,
    pub hash: String,
    pub hash_previo: String,
    pub timestamp: u64,
    pub productor: String,
    pub txs: Vec<Tx>,
}

impl Bloque {
    pub fn total_comisiones(&self) -> u64 {
        self.txs.iter().map(|t| t.comision).sum()
    }
    pub fn volumen(&self) -> u64 {
        self.txs.iter().map(|t| t.monto).sum()
    }
}

/// Lo mínimo que la UI necesita de una cadena.
pub trait FuenteCadena {
    fn altura(&self) -> u64;
    fn bloque(&self, altura: u64) -> Option<&Bloque>;
    fn buscar_hash(&self, prefijo: &str) -> Option<Resultado>;
    fn txs_de_direccion(&self, dir: &str) -> Vec<(u64, Tx)>;
}

/// Resultado de una búsqueda por hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resultado {
    Bloque(u64),
    Tx { altura: u64, indice: usize },
}

/// Cadena generada en memoria con un PRNG determinista (xorshift).
pub struct CadenaSimulada {
    pub bloques: Vec<Bloque>,
}

fn sha(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

impl CadenaSimulada {
    /// Genera `n` bloques con entre 0 y 7 transacciones cada uno.
    pub fn generar(n: u64, semilla: u64) -> Self {
        let mut estado = semilla.max(1);
        let mut rnd = move || {
            estado ^= estado << 13;
            estado ^= estado >> 7;
            estado ^= estado << 17;
            estado
        };
        let cuentas = ["ana", "beto", "caro", "dani", "eli", "fede"];
        let mut bloques: Vec<Bloque> = Vec::new();
        let mut previo = "0".repeat(64);
        for h in 0..n {
            let ntx = (rnd() % 8) as usize;
            let txs: Vec<Tx> = (0..ntx)
                .map(|i| {
                    let de = cuentas[(rnd() % 6) as usize].to_string();
                    let para = cuentas[(rnd() % 6) as usize].to_string();
                    let monto = rnd() % 10_000;
                    Tx { hash: sha(&format!("{h}-{i}-{de}-{para}-{monto}")), de, para, monto, comision: 1 + rnd() % 50 }
                })
                .collect();
            let hash = sha(&format!("{previo}{h}{}", txs.len()));
            bloques.push(Bloque { altura: h, hash: hash.clone(), hash_previo: previo, timestamp: 1_700_000_000 + h * 12, productor: format!("validador-{}", rnd() % 4), txs });
            previo = hash;
        }
        Self { bloques }
    }
}

impl FuenteCadena for CadenaSimulada {
    fn altura(&self) -> u64 {
        self.bloques.len().saturating_sub(1) as u64
    }
    fn bloque(&self, altura: u64) -> Option<&Bloque> {
        self.bloques.get(altura as usize)
    }
    fn buscar_hash(&self, prefijo: &str) -> Option<Resultado> {
        let p = prefijo.trim().to_lowercase();
        if p.len() < 4 {
            return None; // prefijos demasiado cortos serían ambiguos
        }
        for b in &self.bloques {
            if b.hash.starts_with(&p) {
                return Some(Resultado::Bloque(b.altura));
            }
            if let Some(i) = b.txs.iter().position(|t| t.hash.starts_with(&p)) {
                return Some(Resultado::Tx { altura: b.altura, indice: i });
            }
        }
        None
    }
    fn txs_de_direccion(&self, dir: &str) -> Vec<(u64, Tx)> {
        self.bloques.iter().flat_map(|b| b.txs.iter().filter(|t| t.de == dir || t.para == dir).map(move |t| (b.altura, t.clone()))).collect()
    }
}
