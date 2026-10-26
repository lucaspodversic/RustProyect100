//! Reportes HID de teclado ("boot protocol").
//!
//! El reporte de arranque tiene 8 bytes:
//!
//! | byte | contenido |
//! |---|---|
//! | 0 | modificadores (bit 0 LCtrl, 1 LShift, 2 LAlt, 3 LGui, 4..7 lado derecho) |
//! | 1 | reservado (0) |
//! | 2..8 | hasta 6 códigos de tecla pulsados (**6KRO**) |
//!
//! Si hay más de 6 teclas pulsadas, el estándar pide llenar los 6 huecos con
//! `0x01` (*ErrorRollOver*). Así el host sabe que el reporte no es fiable.

/// Bits de modificadores.
pub mod mods {
    /// Control izquierdo.
    pub const LCTRL: u8 = 1 << 0;
    /// Shift izquierdo.
    pub const LSHIFT: u8 = 1 << 1;
    /// Alt izquierdo.
    pub const LALT: u8 = 1 << 2;
    /// GUI (tecla de sistema) izquierda.
    pub const LGUI: u8 = 1 << 3;
}

/// Códigos de uso HID (tabla "Keyboard/Keypad Page 0x07").
#[allow(missing_docs)]
pub mod kc {
    pub const A: u8 = 0x04;
    pub const C: u8 = 0x06;
    pub const S: u8 = 0x16;
    pub const V: u8 = 0x19;
    pub const Z: u8 = 0x1D;
    pub const N1: u8 = 0x1E;
    pub const N0: u8 = 0x27;
    pub const ENTER: u8 = 0x28;
    pub const ESC: u8 = 0x29;
    pub const BORRAR: u8 = 0x2A;
    pub const TAB: u8 = 0x2B;
    pub const ESPACIO: u8 = 0x2C;
    pub const MENOS: u8 = 0x2D;
    pub const PUNTO: u8 = 0x37;
    pub const F1: u8 = 0x3A;
    pub const F13: u8 = 0x68;
    pub const SILENCIO: u8 = 0x7F;
    pub const VOL_ARRIBA: u8 = 0x80;
    pub const VOL_ABAJO: u8 = 0x81;
    /// Código de error de *rollover*.
    pub const ERROR_ROLLOVER: u8 = 0x01;
}

/// Reporte de teclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Reporte {
    /// Byte de modificadores.
    pub modificadores: u8,
    /// Hasta 6 teclas; 0 = hueco libre.
    pub teclas: [u8; 6],
}

impl Reporte {
    /// Añade una tecla. Si no hay hueco, marca *rollover* y devuelve `false`.
    pub fn agregar(&mut self, codigo: u8) -> bool {
        if self.teclas.contains(&codigo) {
            return true;
        }
        match self.teclas.iter_mut().find(|t| **t == 0) {
            Some(h) => {
                *h = codigo;
                true
            }
            None => {
                self.teclas = [kc::ERROR_ROLLOVER; 6];
                false
            }
        }
    }

    /// Serializa a los 8 bytes que viajan por USB.
    pub fn bytes(&self) -> [u8; 8] {
        let t = self.teclas;
        [self.modificadores, 0, t[0], t[1], t[2], t[3], t[4], t[5]]
    }
}

/// Convierte un carácter ASCII imprimible en (código, modificadores) para
/// un sistema con distribución **US**. Devuelve `None` si no está soportado.
pub fn ascii_a_hid(ch: char) -> Option<(u8, u8)> {
    Some(match ch {
        'a'..='z' => (kc::A + (ch as u8 - b'a'), 0),
        'A'..='Z' => (kc::A + (ch as u8 - b'A'), mods::LSHIFT),
        '1'..='9' => (kc::N1 + (ch as u8 - b'1'), 0),
        '0' => (kc::N0, 0),
        ' ' => (kc::ESPACIO, 0),
        '\n' => (kc::ENTER, 0),
        '\t' => (kc::TAB, 0),
        '-' => (kc::MENOS, 0),
        '_' => (kc::MENOS, mods::LSHIFT),
        '.' => (kc::PUNTO, 0),
        '!' => (kc::N1, mods::LSHIFT),
        '@' => (kc::N1 + 1, mods::LSHIFT),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reporte_y_rollover() {
        let mut r = Reporte { modificadores: mods::LCTRL, ..Default::default() };
        assert!(r.agregar(kc::C));
        assert!(r.agregar(kc::C)); // repetida: no ocupa otro hueco
        assert_eq!(r.bytes(), [1, 0, kc::C, 0, 0, 0, 0, 0]);
        for k in 0..5 {
            assert!(r.agregar(kc::N1 + k));
        }
        assert!(!r.agregar(kc::Z)); // séptima tecla
        assert_eq!(r.teclas, [kc::ERROR_ROLLOVER; 6]);
    }

    #[test]
    fn ascii() {
        assert_eq!(ascii_a_hid('h'), Some((0x0B, 0)));
        assert_eq!(ascii_a_hid('H'), Some((0x0B, mods::LSHIFT)));
        assert_eq!(ascii_a_hid('0'), Some((0x27, 0)));
        assert_eq!(ascii_a_hid('@'), Some((0x1F, mods::LSHIFT)));
        assert_eq!(ascii_a_hid('ñ'), None);
    }
}
