//! # Árbol de Merkle (proyecto 71)
//!
//! Un árbol de Merkle resume N transacciones en un único hash (la **raíz**).
//! Para demostrar que una transacción está incluida basta enviar los
//! ~log2(N) hashes "hermanos" del camino hoja→raíz (**prueba de inclusión**).
//! Así funcionan los clientes livianos (SPV): sólo guardan encabezados.
//!
//! Soportamos dos modos:
//!
//! - [`Modo::Rfc6962`] (recomendado): separación de dominio — las hojas se
//!   hashean como `H(0x00 || dato)` y los nodos internos como
//!   `H(0x01 || izq || der)`. Esto impide el ataque de "segunda preimagen",
//!   donde un nodo interno se hace pasar por hoja. Un nodo impar se promueve
//!   sin duplicar.
//! - [`Modo::Bitcoin`]: doble SHA-256 sin prefijos y **duplicando** el último
//!   nodo de un nivel impar. Compatible con la raíz de los bloques de Bitcoin,
//!   pero vulnerable a CVE-2012-2459 (dos listas distintas → misma raíz), lo
//!   que demostramos en los tests.

use sha2::{Digest, Sha256};

pub type Hash = [u8; 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    Rfc6962,
    Bitcoin,
}

/// Lado en el que va el hermano dentro de una prueba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lado {
    Izquierda,
    Derecha,
}

/// Prueba de inclusión: hermanos desde la hoja hacia la raíz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prueba {
    pub indice: usize,
    pub pasos: Vec<(Lado, Hash)>,
}

fn sha256(d: &[u8]) -> Hash {
    Sha256::digest(d).into()
}

impl Modo {
    /// Hash de una hoja a partir de los datos crudos.
    pub fn hash_hoja(self, dato: &[u8]) -> Hash {
        match self {
            Modo::Rfc6962 => {
                let mut v = Vec::with_capacity(dato.len() + 1);
                v.push(0x00);
                v.extend_from_slice(dato);
                sha256(&v)
            }
            Modo::Bitcoin => sha256(&sha256(dato)),
        }
    }

    /// Hash de un nodo interno.
    pub fn hash_nodo(self, izq: &Hash, der: &Hash) -> Hash {
        let mut v = Vec::with_capacity(65);
        if self == Modo::Rfc6962 {
            v.push(0x01);
        }
        v.extend_from_slice(izq);
        v.extend_from_slice(der);
        match self {
            Modo::Rfc6962 => sha256(&v),
            Modo::Bitcoin => sha256(&sha256(&v)),
        }
    }
}

/// Árbol completo guardado por niveles (nivel 0 = hojas).
#[derive(Debug, Clone)]
pub struct ArbolMerkle {
    pub modo: Modo,
    pub niveles: Vec<Vec<Hash>>,
}

impl ArbolMerkle {
    /// Construye a partir de datos crudos (se hashean como hojas).
    pub fn desde_datos<T: AsRef<[u8]>>(modo: Modo, datos: &[T]) -> Self {
        let hojas = datos.iter().map(|d| modo.hash_hoja(d.as_ref())).collect();
        Self::desde_hojas(modo, hojas)
    }

    /// Construye a partir de hashes de hoja ya calculados (p. ej. txids).
    pub fn desde_hojas(modo: Modo, hojas: Vec<Hash>) -> Self {
        let mut niveles = vec![hojas];
        while niveles.last().unwrap().len() > 1 {
            let actual = niveles.last().unwrap();
            let mut sig = Vec::with_capacity(actual.len().div_ceil(2));
            for par in actual.chunks(2) {
                match (par, modo) {
                    ([a, b], _) => sig.push(modo.hash_nodo(a, b)),
                    ([a], Modo::Bitcoin) => sig.push(modo.hash_nodo(a, a)), // duplicar
                    ([a], Modo::Rfc6962) => sig.push(*a),                     // promover
                    _ => unreachable!(),
                }
            }
            niveles.push(sig);
        }
        Self { modo, niveles }
    }

    pub fn cantidad_hojas(&self) -> usize {
        self.niveles[0].len()
    }

    /// Raíz. Para un árbol vacío devolvemos `sha256("")` (convención RFC 6962).
    pub fn raiz(&self) -> Hash {
        match self.niveles.last().and_then(|n| n.first()) {
            Some(h) => *h,
            None => sha256(b""),
        }
    }

    /// Genera la prueba de inclusión de la hoja `indice`.
    pub fn prueba(&self, mut indice: usize) -> Option<Prueba> {
        if indice >= self.cantidad_hojas() {
            return None;
        }
        let original = indice;
        let mut pasos = vec![];
        for nivel in &self.niveles[..self.niveles.len() - 1] {
            let hermano = indice ^ 1;
            if hermano < nivel.len() {
                let lado = if indice % 2 == 0 { Lado::Derecha } else { Lado::Izquierda };
                pasos.push((lado, nivel[hermano]));
            } else if self.modo == Modo::Bitcoin {
                // Nodo impar duplicado: el hermano es él mismo.
                pasos.push((Lado::Derecha, nivel[indice]));
            }
            // En RFC 6962 el nodo impar sube sin paso extra.
            indice /= 2;
        }
        Some(Prueba { indice: original, pasos })
    }

    /// Actualiza una hoja y recalcula sólo el camino hasta la raíz: O(log n).
    pub fn actualizar_hoja(&mut self, mut indice: usize, nuevo: Hash) -> bool {
        if indice >= self.cantidad_hojas() {
            return false;
        }
        self.niveles[0][indice] = nuevo;
        for n in 1..self.niveles.len() {
            let padre = indice / 2;
            let izq = self.niveles[n - 1][padre * 2];
            let valor = match self.niveles[n - 1].get(padre * 2 + 1) {
                Some(der) => self.modo.hash_nodo(&izq, der),
                None if self.modo == Modo::Bitcoin => self.modo.hash_nodo(&izq, &izq),
                None => izq,
            };
            self.niveles[n][padre] = valor;
            indice = padre;
        }
        true
    }
}

/// Verifica una prueba: recorre los pasos recomputando hasta la raíz.
pub fn verificar(modo: Modo, hoja: &Hash, prueba: &Prueba, raiz: &Hash) -> bool {
    let mut acc = *hoja;
    for (lado, h) in &prueba.pasos {
        acc = match lado {
            Lado::Derecha => modo.hash_nodo(&acc, h),
            Lado::Izquierda => modo.hash_nodo(h, &acc),
        };
    }
    acc == *raiz
}

/// Convierte un txid en el orden "de pantalla" de Bitcoin (hex invertido)
/// al orden interno de bytes.
pub fn desde_hex_invertido(s: &str) -> Option<Hash> {
    let mut b: Hash = hex::decode(s).ok()?.try_into().ok()?;
    b.reverse();
    Some(b)
}

/// Inverso de [`desde_hex_invertido`].
pub fn a_hex_invertido(h: &Hash) -> String {
    let mut b = *h;
    b.reverse();
    hex::encode(b)
}

/// Cliente liviano (SPV): sólo conoce raíces de encabezados confiables.
#[derive(Debug, Default)]
pub struct ClienteSpv {
    pub raices: Vec<Hash>,
}

impl ClienteSpv {
    /// ¿La tx `hoja` está incluida en el bloque `altura`?
    pub fn tx_confirmada(&self, modo: Modo, altura: usize, hoja: &Hash, prueba: &Prueba) -> bool {
        self.raices.get(altura).is_some_and(|r| verificar(modo, hoja, prueba, r))
    }
}
