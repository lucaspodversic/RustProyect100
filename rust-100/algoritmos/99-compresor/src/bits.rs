//! Escritura y lectura de bits, del más significativo al menos significativo.

use crate::Error;

/// Acumula bits en un `u64` y los vuelca byte a byte.
#[derive(Debug, Default)]
pub struct EscritorBits {
    buf: Vec<u8>,
    acc: u64,
    n: u32, // bits válidos en acc
}

impl EscritorBits {
    /// Nuevo escritor vacío.
    pub fn new() -> Self {
        Self::default()
    }

    /// Escribe los `len` bits menos significativos de `codigo` (len ≤ 32),
    /// empezando por el más alto.
    pub fn escribir(&mut self, codigo: u32, len: u32) {
        debug_assert!(len <= 32);
        self.acc = (self.acc << len) | (codigo as u64 & ((1u64 << len) - 1));
        self.n += len;
        while self.n >= 8 {
            self.n -= 8;
            self.buf.push((self.acc >> self.n) as u8);
        }
    }

    /// Termina: rellena el último byte con ceros y devuelve el buffer.
    pub fn terminar(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.buf.push((self.acc << (8 - self.n)) as u8);
        }
        self.buf
    }
}

/// Lee bits de un slice.
#[derive(Debug)]
pub struct LectorBits<'a> {
    datos: &'a [u8],
    pos: usize, // posición en bits
}

impl<'a> LectorBits<'a> {
    /// Nuevo lector.
    pub fn new(datos: &'a [u8]) -> Self {
        LectorBits { datos, pos: 0 }
    }

    /// Lee un bit (`Error::Truncado` al final).
    pub fn bit(&mut self) -> Result<u32, Error> {
        let byte = *self.datos.get(self.pos / 8).ok_or(Error::Truncado)?;
        let b = (byte >> (7 - self.pos % 8)) & 1;
        self.pos += 1;
        Ok(b as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta() {
        let mut w = EscritorBits::new();
        w.escribir(0b101, 3);
        w.escribir(0b1, 1);
        w.escribir(0b0000_1111_0, 9);
        let v = w.terminar();
        assert_eq!(v, vec![0b1011_0000, 0b1111_0000]);
        let mut r = LectorBits::new(&v);
        let bits: Vec<u32> = (0..13).map(|_| r.bit().unwrap()).collect();
        assert_eq!(bits, [1, 0, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1, 0]);
    }
}
