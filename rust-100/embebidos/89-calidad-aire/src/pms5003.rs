//! Parser de tramas del sensor de partículas **PMS5003**.
//!
//! ## Formato (32 bytes, big‑endian)
//! ```text
//! 0x42 0x4D | len(2)=28 | 13 palabras de datos (26 bytes) | checksum(2)
//! ```
//! El checksum es la **suma de los 30 bytes anteriores**. Las palabras que
//! nos interesan son PM1.0/PM2.5/PM10 "atmosféricas" (índices 3, 4 y 5),
//! que el fabricante recomienda para ambientes exteriores/interiores normales.
//!
//! El parser es una máquina de estados que consume **de a un byte**: así
//! puede alimentarse desde una interrupción de UART y se resincroniza solo si
//! entra a mitad de una trama (busca de nuevo `0x42 0x4D`).

use embedded_io::Read;

/// Largo total de una trama.
pub const LARGO_TRAMA: usize = 32;

/// Datos útiles de una trama.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Trama {
    /// PM1.0 en µg/m³ (ambiente atmosférico).
    pub pm1_0: u16,
    /// PM2.5 en µg/m³.
    pub pm2_5: u16,
    /// PM10 en µg/m³.
    pub pm10: u16,
    /// Partículas > 0,3 µm por cada 0,1 L de aire.
    pub particulas_0_3: u16,
}

/// Errores de parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorTrama {
    /// El checksum no coincide.
    Checksum {
        /// Calculado.
        calculado: u16,
        /// Recibido.
        recibido: u16,
    },
    /// El campo largo no vale 28.
    Largo(u16),
}

/// Parser incremental.
pub struct Parser {
    buf: [u8; LARGO_TRAMA],
    pos: usize,
}

impl Default for Parser {
    fn default() -> Self {
        Self { buf: [0; LARGO_TRAMA], pos: 0 }
    }
}

impl Parser {
    /// Alimenta un byte. Devuelve `Some(..)` cuando completó una trama.
    pub fn alimentar(&mut self, b: u8) -> Option<Result<Trama, ErrorTrama>> {
        match self.pos {
            // Esperando el primer byte de inicio.
            0 => {
                if b == 0x42 {
                    self.buf[0] = b;
                    self.pos = 1;
                }
                None
            }
            // Segundo byte de inicio; si no es 0x4D, resincronizamos (y el
            // byte actual podría ser a su vez un 0x42 válido).
            1 => {
                if b == 0x4D {
                    self.buf[1] = b;
                    self.pos = 2;
                } else {
                    self.pos = if b == 0x42 { 1 } else { 0 };
                }
                None
            }
            _ => {
                self.buf[self.pos] = b;
                self.pos += 1;
                if self.pos < LARGO_TRAMA {
                    return None;
                }
                self.pos = 0;
                Some(decodificar(&self.buf))
            }
        }
    }
}

/// Lee una palabra big‑endian en el offset `i`.
fn palabra(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([b[i], b[i + 1]])
}

/// Decodifica una trama completa ya alineada.
pub fn decodificar(b: &[u8; LARGO_TRAMA]) -> Result<Trama, ErrorTrama> {
    let largo = palabra(b, 2);
    if largo != 28 {
        return Err(ErrorTrama::Largo(largo));
    }
    let calculado = b[..30].iter().map(|&x| x as u16).fold(0u16, |a, x| a.wrapping_add(x));
    let recibido = palabra(b, 30);
    if calculado != recibido {
        return Err(ErrorTrama::Checksum { calculado, recibido });
    }
    // Datos: palabra k está en 4 + 2k.
    Ok(Trama {
        pm1_0: palabra(b, 4 + 2 * 3),
        pm2_5: palabra(b, 4 + 2 * 4),
        pm10: palabra(b, 4 + 2 * 5),
        particulas_0_3: palabra(b, 4 + 2 * 6),
    })
}

/// Lee de la UART hasta obtener una trama válida (o error de E/S).
/// Las tramas corruptas se descartan en silencio y se sigue leyendo.
pub fn leer_trama<R: Read>(uart: &mut R, parser: &mut Parser) -> Result<Trama, R::Error> {
    let mut byte = [0u8; 1];
    loop {
        let n = uart.read(&mut byte)?;
        if n == 0 {
            continue; // sin datos todavía (UART no bloqueante)
        }
        if let Some(Ok(t)) = parser.alimentar(byte[0]) {
            return Ok(t);
        }
    }
}

/// Construye una trama válida (para tests y simulación en host).
pub fn construir_trama(pm1: u16, pm25: u16, pm10: u16) -> [u8; LARGO_TRAMA] {
    let mut b = [0u8; LARGO_TRAMA];
    b[0] = 0x42;
    b[1] = 0x4D;
    b[2..4].copy_from_slice(&28u16.to_be_bytes());
    // Palabras 0..2 = valores "CF=1"; 3..5 = atmosféricos (usamos los mismos).
    for (k, v) in [pm1, pm25, pm10, pm1, pm25, pm10].iter().enumerate() {
        b[4 + 2 * k..6 + 2 * k].copy_from_slice(&v.to_be_bytes());
    }
    b[16..18].copy_from_slice(&(pm25 * 100).to_be_bytes()); // partículas > 0,3 µm
    let suma = b[..30].iter().map(|&x| x as u16).fold(0u16, |a, x| a.wrapping_add(x));
    b[30..32].copy_from_slice(&suma.to_be_bytes());
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodifica_trama_valida() {
        let t = decodificar(&construir_trama(5, 12, 20)).unwrap();
        assert_eq!((t.pm1_0, t.pm2_5, t.pm10, t.particulas_0_3), (5, 12, 20, 1200));
    }

    #[test]
    fn detecta_checksum_y_largo() {
        let mut b = construir_trama(5, 12, 20);
        b[10] ^= 1;
        assert!(matches!(decodificar(&b), Err(ErrorTrama::Checksum { .. })));
        let mut b = construir_trama(5, 12, 20);
        b[3] = 20;
        assert_eq!(decodificar(&b), Err(ErrorTrama::Largo(20)));
    }

    #[test]
    fn resincroniza_con_basura_y_lee_de_uart() {
        // Basura + inicio falso "0x42 0x42 0x4D..." + trama válida.
        let mut flujo = vec![0x00, 0xFF, 0x42, 0x13, 0x42];
        flujo.extend_from_slice(&construir_trama(1, 2, 3)[1..]); // empieza con el 2º 0x42
        flujo.extend_from_slice(&construir_trama(7, 8, 9));
        let mut uart: &[u8] = &flujo;
        let mut p = Parser::default();
        assert_eq!(leer_trama(&mut uart, &mut p).unwrap().pm2_5, 2);
        assert_eq!(leer_trama(&mut uart, &mut p).unwrap().pm2_5, 8);
    }
}
