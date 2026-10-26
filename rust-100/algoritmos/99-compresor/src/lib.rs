//! # Compresor RLE + Huffman
//!
//! - [`rle`]: *run‑length encoding* estilo **PackBits** (el de TIFF y Mac):
//!   alterna bloques literales y repeticiones, así que en el peor caso solo
//!   crece 1 byte por cada 128.
//! - [`bits`]: escritor y lector de bits (MSB primero).
//! - [`huffman`]: Huffman **canónico** con longitud máxima de 15 bits. Basta
//!   con guardar las longitudes de los códigos (256 bytes), no el árbol.
//! - [`contenedor`]: formato de archivo `CMP1` (método + tamaño + CRC‑32) y
//!   selección automática del método que dé el resultado más pequeño.
//!
//! Todas las funciones de descompresión devuelven `Result`: con datos
//! corruptos se obtiene un error y nunca un *panic*.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod bits;
pub mod contenedor;
pub mod crc32;
pub mod huffman;
pub mod rle;

/// Errores de descompresión.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Los datos terminan antes de lo esperado.
    Truncado,
    /// Cabecera o número mágico incorrectos.
    Formato(&'static str),
    /// El CRC no coincide: los datos están corruptos.
    Crc {
        /// CRC guardado.
        esperado: u32,
        /// CRC calculado.
        obtenido: u32,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Truncado => write!(f, "datos truncados"),
            Error::Formato(m) => write!(f, "formato inválido: {m}"),
            Error::Crc { esperado, obtenido } => {
                write!(f, "CRC incorrecto (esperado {esperado:08x}, obtenido {obtenido:08x})")
            }
        }
    }
}

impl std::error::Error for Error {}
