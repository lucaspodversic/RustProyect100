//! Formato de archivo `CMP1`.
//!
//! ```text
//! offset  tamaño  campo
//! 0       4       "CMP1"
//! 4       1       método (0 = almacenado, 1 = RLE, 2 = Huffman, 3 = RLE+Huffman)
//! 5       8       tamaño original (u64 LE)
//! 13      4       CRC‑32 del original (u32 LE)
//! 17      ...     carga útil
//! ```
//!
//! El CRC detecta corrupción **después** de descomprimir. Además, el tamaño
//! se comprueba por separado.

use crate::{crc32::crc32, huffman, rle, Error};

/// Métodos de compresión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metodo {
    /// Sin compresión (para datos incompresibles: así nunca crece más de 17 bytes).
    Almacenado = 0,
    /// Solo RLE.
    Rle = 1,
    /// Solo Huffman.
    Huffman = 2,
    /// RLE y después Huffman sobre el resultado.
    RleHuffman = 3,
}

impl Metodo {
    /// Todos los métodos.
    pub const TODOS: [Metodo; 4] = [Metodo::Almacenado, Metodo::Rle, Metodo::Huffman, Metodo::RleHuffman];

    fn desde(b: u8) -> Result<Self, Error> {
        Self::TODOS.get(b as usize).copied().ok_or(Error::Formato("método desconocido"))
    }
}

const MAGICO: &[u8; 4] = b"CMP1";
const CABECERA: usize = 17;

fn carga(datos: &[u8], m: Metodo) -> Vec<u8> {
    match m {
        Metodo::Almacenado => datos.to_vec(),
        Metodo::Rle => rle::comprimir(datos),
        Metodo::Huffman => huffman::comprimir(datos),
        Metodo::RleHuffman => huffman::comprimir(&rle::comprimir(datos)),
    }
}

/// Comprime con un método concreto.
pub fn comprimir_con(datos: &[u8], m: Metodo) -> Vec<u8> {
    let cuerpo = carga(datos, m);
    let mut s = Vec::with_capacity(CABECERA + cuerpo.len());
    s.extend_from_slice(MAGICO);
    s.push(m as u8);
    s.extend_from_slice(&(datos.len() as u64).to_le_bytes());
    s.extend_from_slice(&crc32(datos).to_le_bytes());
    s.extend(cuerpo);
    s
}

/// Prueba todos los métodos y se queda con el más pequeño.
pub fn comprimir(datos: &[u8]) -> Vec<u8> {
    Metodo::TODOS
        .iter()
        .map(|&m| comprimir_con(datos, m))
        .min_by_key(Vec::len)
        .expect("hay métodos")
}

/// Lee la cabecera: (método, tamaño original, crc).
pub fn info(datos: &[u8]) -> Result<(Metodo, u64, u32), Error> {
    if datos.len() < CABECERA {
        return Err(Error::Truncado);
    }
    if &datos[..4] != MAGICO {
        return Err(Error::Formato("número mágico incorrecto (no es CMP1)"));
    }
    Ok((
        Metodo::desde(datos[4])?,
        u64::from_le_bytes(datos[5..13].try_into().unwrap()),
        u32::from_le_bytes(datos[13..17].try_into().unwrap()),
    ))
}

/// Descomprime y verifica tamaño y CRC.
pub fn descomprimir(datos: &[u8]) -> Result<Vec<u8>, Error> {
    let (m, n, crc) = info(datos)?;
    let cuerpo = &datos[CABECERA..];
    let sal = match m {
        Metodo::Almacenado => cuerpo.to_vec(),
        Metodo::Rle => rle::descomprimir(cuerpo)?,
        Metodo::Huffman => huffman::descomprimir(cuerpo)?,
        Metodo::RleHuffman => rle::descomprimir(&huffman::descomprimir(cuerpo)?)?,
    };
    if sal.len() as u64 != n {
        return Err(Error::Formato("el tamaño no coincide"));
    }
    let obtenido = crc32(&sal);
    if obtenido != crc {
        return Err(Error::Crc { esperado: crc, obtenido });
    }
    Ok(sal)
}
