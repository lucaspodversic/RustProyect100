//! CRC‑32 (IEEE 802.3, el mismo de ZIP, PNG y gzip).
//!
//! Se usa el polinomio reflejado 0xEDB88320 con una tabla de 256 entradas
//! que se calcula **en tiempo de compilación** (`const fn`): procesa un byte
//! por iteración en lugar de un bit.

const fn tabla() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

static TABLA: [u32; 256] = tabla();

/// Calcula el CRC‑32 de `datos`.
pub fn crc32(datos: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in datos {
        c = TABLA[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

#[cfg(test)]
mod tests {
    #[test]
    fn vector_estandar() {
        // Valor de verificación conocido ("check") del CRC‑32.
        assert_eq!(super::crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(super::crc32(b""), 0);
    }
}
