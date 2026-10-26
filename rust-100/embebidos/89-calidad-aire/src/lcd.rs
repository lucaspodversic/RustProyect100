//! Driver de LCD de caracteres **HD44780** a través de un "backpack" I2C
//! **PCF8574** (el módulo azul típico de 16×2 con 4 pines).
//!
//! El PCF8574 expone 8 salidas; el cableado estándar es:
//! ```text
//! P0=RS  P1=RW  P2=EN  P3=Backlight  P4..P7 = D4..D7
//! ```
//! El HD44780 se usa en **modo 4 bits**: cada byte se manda como dos nibbles
//! (alto primero) y cada nibble se "latchea" con un pulso en EN.

use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;

/// Dirección típica del PCF8574 (0x3F en la variante PCF8574A).
pub const DIRECCION: u8 = 0x27;
const RS: u8 = 0x01;
const EN: u8 = 0x04;
const BL: u8 = 0x08;

/// Driver del LCD.
pub struct Lcd<I2C> {
    i2c: I2C,
    dir: u8,
}

impl<I2C: I2c> Lcd<I2C> {
    /// Crea el driver.
    pub fn new(i2c: I2C, dir: u8) -> Self {
        Self { i2c, dir }
    }
    /// Devuelve el bus.
    pub fn liberar(self) -> I2C {
        self.i2c
    }

    /// Escribe un nibble (ya en los 4 bits altos) con un pulso de EN.
    fn nibble(&mut self, n: u8, rs: bool) -> Result<(), I2C::Error> {
        let base = n | BL | if rs { RS } else { 0 };
        // EN alto y luego bajo en una sola transacción: el flanco de bajada
        // latchea el dato (el I2C a 100 kHz ya da >1 µs de ancho de pulso).
        self.i2c.write(self.dir, &[base | EN, base])
    }

    /// Envía un byte completo (comando si `rs=false`, dato si `rs=true`).
    pub fn byte(&mut self, b: u8, rs: bool) -> Result<(), I2C::Error> {
        self.nibble(b & 0xF0, rs)?;
        self.nibble((b << 4) & 0xF0, rs)
    }

    /// Secuencia de inicialización "por instrucción" de la hoja de datos:
    /// tres veces 0x3 (modo 8 bits) y luego 0x2 para pasar a 4 bits.
    pub fn iniciar<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), I2C::Error> {
        delay.delay_ms(50);
        for _ in 0..3 {
            self.nibble(0x30, false)?;
            delay.delay_ms(5);
        }
        self.nibble(0x20, false)?;
        self.byte(0x28, false)?; // 4 bits, 2 líneas, fuente 5×8
        self.byte(0x0C, false)?; // display ON, cursor OFF
        self.byte(0x06, false)?; // incremento automático
        self.byte(0x01, false)?; // borrar
        delay.delay_ms(2);
        Ok(())
    }

    /// Posiciona el cursor (fila 0 o 1).
    pub fn cursor(&mut self, col: u8, fila: u8) -> Result<(), I2C::Error> {
        let base = if fila == 0 { 0x00 } else { 0x40 };
        self.byte(0x80 | (base + col), false)
    }

    /// Escribe una línea completa de 16 columnas (rellena con espacios).
    pub fn linea(&mut self, fila: u8, texto: &[u8; 16]) -> Result<(), I2C::Error> {
        self.cursor(0, fila)?;
        for &c in texto {
            self.byte(c, true)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::i2c::{Mock, Transaction};

    /// Transacciones esperadas para un byte.
    fn tx_byte(b: u8, rs: bool) -> Vec<Transaction> {
        [b & 0xF0, (b << 4) & 0xF0]
            .iter()
            .map(|&n| {
                let base = n | BL | if rs { RS } else { 0 };
                Transaction::write(DIRECCION, vec![base | EN, base])
            })
            .collect()
    }

    #[test]
    fn escribe_caracter_en_dos_nibbles() {
        let tx = tx_byte(b'A', true);
        assert_eq!(tx.len(), 2);
        let mut lcd = Lcd::new(Mock::new(&tx), DIRECCION);
        lcd.byte(b'A', true).unwrap();
        lcd.liberar().done();
    }

    #[test]
    fn cursor_segunda_fila() {
        let tx = tx_byte(0xC3, false);
        let mut lcd = Lcd::new(Mock::new(&tx), DIRECCION);
        lcd.cursor(3, 1).unwrap();
        lcd.liberar().done();
    }
}
