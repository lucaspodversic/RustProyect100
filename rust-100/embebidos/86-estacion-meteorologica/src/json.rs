//! Serialización mínima a JSON sobre un buffer de tamaño fijo.
//!
//! `serde_json` necesita `alloc`; para un firmware chico alcanza con
//! implementar `core::fmt::Write` sobre un `[u8; N]` y escribir con
//! `write!`. Si el texto no entra, se devuelve `fmt::Error` en lugar de
//! truncar silenciosamente (un JSON truncado sería inválido).

use core::fmt::{self, Write};

/// Buffer de texto con capacidad fija `N`.
pub struct BufferFijo<const N: usize> {
    datos: [u8; N],
    largo: usize,
}

impl<const N: usize> Default for BufferFijo<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> BufferFijo<N> {
    /// Crea un buffer vacío.
    pub const fn new() -> Self {
        Self { datos: [0; N], largo: 0 }
    }

    /// Contenido como `&str` (siempre es UTF‑8 válido porque solo se escribe
    /// vía `write_str`, que recibe `&str`).
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.datos[..self.largo]).unwrap_or("")
    }

    /// Contenido como bytes (para mandarlo por la red).
    pub fn as_bytes(&self) -> &[u8] {
        &self.datos[..self.largo]
    }

    /// Vacía el buffer.
    pub fn limpiar(&mut self) {
        self.largo = 0;
    }
}

impl<const N: usize> Write for BufferFijo<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let b = s.as_bytes();
        if self.largo + b.len() > N {
            return Err(fmt::Error);
        }
        self.datos[self.largo..self.largo + b.len()].copy_from_slice(b);
        self.largo += b.len();
        Ok(())
    }
}

/// Envoltorio que formatea un número en centésimas como decimal con 2
/// cifras: `2153` → `21.53`, `-5` → `-0.05`.
pub struct Centesimas(pub i32);

impl fmt::Display for Centesimas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let signo = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        write!(f, "{}{}.{:02}", signo, abs / 100, abs % 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escribe_y_formatea() {
        let mut b = BufferFijo::<32>::new();
        write!(b, "{{\"t\":{}}}", Centesimas(2153)).unwrap();
        assert_eq!(b.as_str(), "{\"t\":21.53}");
    }

    #[test]
    fn negativos_chicos() {
        let mut b = BufferFijo::<16>::new();
        write!(b, "{}", Centesimas(-5)).unwrap();
        assert_eq!(b.as_str(), "-0.05");
    }

    #[test]
    fn desborde_es_error() {
        let mut b = BufferFijo::<4>::new();
        assert!(write!(b, "12345").is_err());
    }
}
