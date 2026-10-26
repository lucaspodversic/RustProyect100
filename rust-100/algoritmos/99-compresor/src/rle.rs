//! RLE estilo PackBits.
//!
//! Cada bloque empieza por un byte de cabecera `n` (interpretado como i8):
//!
//! | n | Significado |
//! |---|---|
//! | 0..=127 | los siguientes `n+1` bytes son literales |
//! | -1..=-127 | el siguiente byte se repite `1-n` veces (2..=128) |
//! | -128 | no se usa (se ignora, como en el estándar) |
//!
//! Solo vale la pena codificar como repetición las rachas de 3 o más bytes:
//! una racha de 2 en mitad de literales cuesta lo mismo y además rompe el
//! bloque literal.

use crate::Error;

/// Comprime con PackBits.
pub fn comprimir(datos: &[u8]) -> Vec<u8> {
    let mut sal = Vec::with_capacity(datos.len() + datos.len() / 128 + 1);
    let mut i = 0;
    let mut literales_desde = 0; // inicio del bloque literal pendiente

    // Vuelca literales[desde..hasta] en bloques de como máximo 128.
    fn volcar(sal: &mut Vec<u8>, lit: &[u8]) {
        for trozo in lit.chunks(128) {
            sal.push((trozo.len() - 1) as u8);
            sal.extend_from_slice(trozo);
        }
    }

    while i < datos.len() {
        // Longitud de la racha que empieza en i (máximo 128).
        let mut racha = 1;
        while i + racha < datos.len() && racha < 128 && datos[i + racha] == datos[i] {
            racha += 1;
        }
        if racha >= 3 {
            volcar(&mut sal, &datos[literales_desde..i]);
            sal.push((1i16 - racha as i16) as i8 as u8); // -(racha-1)
            sal.push(datos[i]);
            i += racha;
            literales_desde = i;
        } else {
            i += racha;
        }
    }
    volcar(&mut sal, &datos[literales_desde..]);
    sal
}

/// Descomprime PackBits. Error si un bloque está truncado.
pub fn descomprimir(datos: &[u8]) -> Result<Vec<u8>, Error> {
    let mut sal = Vec::with_capacity(datos.len() * 2);
    let mut i = 0;
    while i < datos.len() {
        let n = datos[i] as i8;
        i += 1;
        match n {
            0..=127 => {
                let len = n as usize + 1;
                let lit = datos.get(i..i + len).ok_or(Error::Truncado)?;
                sal.extend_from_slice(lit);
                i += len;
            }
            -127..=-1 => {
                let b = *datos.get(i).ok_or(Error::Truncado)?;
                sal.extend(std::iter::repeat(b).take((1 - n as i16) as usize));
                i += 1;
            }
            -128 => {} // no‑op
        }
    }
    Ok(sal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ejemplo_clasico() {
        let d = b"AAAAAAABCDDDDE";
        let c = comprimir(d);
        // 7×A → [-6, A]; "BC" literal → [1, B, C]; 4×D → [-3, D]; "E" → [0, E]
        assert_eq!(c, vec![0xFA, b'A', 1, b'B', b'C', 0xFD, b'D', 0, b'E']);
        assert_eq!(descomprimir(&c).unwrap(), d);
    }

    #[test]
    fn peor_caso_acotado() {
        let d: Vec<u8> = (0..1000u32).map(|i| (i * 7 % 251) as u8).collect();
        let c = comprimir(&d);
        assert!(c.len() <= d.len() + d.len().div_ceil(128));
        assert_eq!(descomprimir(&c).unwrap(), d);
    }

    #[test]
    fn truncado() {
        assert_eq!(descomprimir(&[5, 1, 2]), Err(Error::Truncado));
        assert_eq!(descomprimir(&[0xFD]), Err(Error::Truncado));
    }
}
