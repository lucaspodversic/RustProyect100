//! Cadena de bloques compacta usada por el nodo.
//!
//! Es una versión reducida del proyecto 66 (PoW + validación), con `serde`
//! para poder enviar bloques por la red.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Bloque serializable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bloque {
    pub indice: u64,
    pub timestamp: u64,
    pub datos: String,
    pub hash_previo: String,
    pub nonce: u64,
    pub hash: String,
}

/// Dificultad fija de la red (ceros hex iniciales). Baja para tests rápidos.
pub const DIFICULTAD: usize = 2;

impl Bloque {
    /// SHA-256 del encabezado (todo menos `hash`).
    pub fn calcular_hash(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.indice.to_le_bytes());
        h.update(self.timestamp.to_le_bytes());
        h.update((self.datos.len() as u64).to_le_bytes());
        h.update(self.datos.as_bytes());
        h.update(self.hash_previo.as_bytes());
        h.update(self.nonce.to_le_bytes());
        hex::encode(h.finalize())
    }

    /// Busca un nonce que cumpla la dificultad de la red.
    pub fn minar(&mut self) {
        self.nonce = 0;
        loop {
            let c = self.calcular_hash();
            if c.as_bytes()[..DIFICULTAD].iter().all(|b| *b == b'0') {
                self.hash = c;
                return;
            }
            self.nonce += 1;
        }
    }

    /// Valida el bloque de forma aislada (hash + PoW).
    pub fn es_valido(&self) -> bool {
        self.hash == self.calcular_hash() && self.hash.as_bytes()[..DIFICULTAD].iter().all(|b| *b == b'0')
    }

    /// El génesis es idéntico en todos los nodos: así comparten la raíz.
    pub fn genesis() -> Self {
        let mut g = Bloque { indice: 0, timestamp: 0, datos: "génesis-red-67".into(), hash_previo: "0".repeat(64), nonce: 0, hash: String::new() };
        g.minar();
        g
    }

    /// Indica si `self` puede ir inmediatamente después de `previo`.
    pub fn sigue_a(&self, previo: &Bloque) -> bool {
        self.indice == previo.indice + 1 && self.hash_previo == previo.hash && self.es_valido()
    }
}

/// Valida una cadena completa partiendo del génesis canónico.
pub fn cadena_valida(bloques: &[Bloque]) -> bool {
    match bloques.first() {
        Some(g) if *g == Bloque::genesis() => bloques.windows(2).all(|w| w[1].sigue_a(&w[0])),
        _ => false,
    }
}
