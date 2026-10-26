//! Generador pseudoaleatorio mínimo (xorshift64) para tests y ejemplos, así
//! el crate no necesita dependencias externas.

/// Generador xorshift64 (Marsaglia, 2003). No es criptográfico.
#[derive(Debug, Clone)]
pub struct XorShift(u64);

impl XorShift {
    /// Crea el generador (la semilla 0 se reemplaza: con 0 siempre daría 0).
    pub fn new(semilla: u64) -> Self {
        Self(if semilla == 0 { 0x9E37_79B9_7F4A_7C15 } else { semilla })
    }

    /// Siguiente número de 64 bits.
    pub fn siguiente(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Número en `0..n`.
    pub fn rango(&mut self, n: u64) -> u64 {
        self.siguiente() % n
    }
}
