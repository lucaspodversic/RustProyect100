//! Colores y operaciones sobre ellos (todo con enteros de 8/16 bits).

/// Color en espacio RGB, 8 bits por canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    /// Rojo.
    pub r: u8,
    /// Verde.
    pub g: u8,
    /// Azul.
    pub b: u8,
}

impl Rgb {
    /// Constructor corto.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
    /// Negro (apagado).
    pub const NEGRO: Rgb = Rgb::new(0, 0, 0);

    /// Escala el color por un brillo 0..=255 (255 = sin cambios).
    pub fn escalar(self, brillo: u8) -> Rgb {
        // (c * b + 127) / 255 redondea al más cercano.
        let f = |c: u8| ((c as u16 * brillo as u16 + 127) / 255) as u8;
        Rgb::new(f(self.r), f(self.g), f(self.b))
    }

    /// Interpolación lineal: `t` en 0..=255 (0 → `self`, 255 → `otro`).
    pub fn mezclar(self, otro: Rgb, t: u8) -> Rgb {
        let f = |a: u8, b: u8| {
            let a = a as i32;
            let b = b as i32;
            (a + ((b - a) * t as i32 + if b >= a { 127 } else { -127 }) / 255) as u8
        };
        Rgb::new(f(self.r, otro.r), f(self.g, otro.g), f(self.b, otro.b))
    }

    /// Acerca `self` hacia `objetivo` como máximo `paso` unidades por canal.
    /// Es la base del fundido suave (evita saltos bruscos de color).
    pub fn acercar(self, objetivo: Rgb, paso: u8) -> Rgb {
        let f = |a: u8, b: u8| {
            if a < b {
                a.saturating_add(paso).min(b)
            } else {
                a.saturating_sub(paso).max(b)
            }
        };
        Rgb::new(f(self.r, objetivo.r), f(self.g, objetivo.g), f(self.b, objetivo.b))
    }
}

/// Color en espacio HSV: matiz 0..360, saturación y valor 0..=255.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hsv {
    /// Matiz en grados (0 = rojo, 120 = verde, 240 = azul).
    pub h: u16,
    /// Saturación.
    pub s: u8,
    /// Valor (brillo).
    pub v: u8,
}

impl Hsv {
    /// Conversión HSV → RGB con enteros (algoritmo por sectores de 60°).
    pub fn a_rgb(self) -> Rgb {
        if self.s == 0 {
            return Rgb::new(self.v, self.v, self.v); // gris
        }
        let h = self.h % 360;
        let sector = h / 60; // 0..5
        let resto = (h % 60) as u32 * 255 / 60; // posición dentro del sector 0..255
        let v = self.v as u32;
        let s = self.s as u32;
        // p, q, t de la fórmula clásica, escalados a 0..255.
        let p = (v * (255 - s) / 255) as u8;
        let q = (v * (255 - s * resto / 255) / 255) as u8;
        let t = (v * (255 - s * (255 - resto) / 255) / 255) as u8;
        let v = v as u8;
        match sector {
            0 => Rgb::new(v, t, p),
            1 => Rgb::new(q, v, p),
            2 => Rgb::new(p, v, t),
            3 => Rgb::new(p, q, v),
            4 => Rgb::new(t, p, v),
            _ => Rgb::new(v, p, q),
        }
    }
}

/// Corrección gamma ≈ 2,0: el ojo percibe el brillo de forma no lineal, así
/// que un PWM al 50 % se ve casi "al 75 %". Elevando al cuadrado conseguimos
/// que los fundidos se perciban parejos. Devuelve 0..=65535 para tener
/// resolución extra en PWM de 16 bits.
pub fn gamma16(c: u8) -> u16 {
    let c = c as u32;
    // c² escalado: 255² = 65025 → lo llevamos a 65535.
    ((c * c * 65535 + 32512) / 65025) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_primarios() {
        assert_eq!(Hsv { h: 0, s: 255, v: 255 }.a_rgb(), Rgb::new(255, 0, 0));
        assert_eq!(Hsv { h: 120, s: 255, v: 255 }.a_rgb(), Rgb::new(0, 255, 0));
        assert_eq!(Hsv { h: 240, s: 255, v: 255 }.a_rgb(), Rgb::new(0, 0, 255));
        assert_eq!(Hsv { h: 60, s: 0, v: 100 }.a_rgb(), Rgb::new(100, 100, 100));
    }

    #[test]
    fn mezcla_extremos_y_mitad() {
        let a = Rgb::new(0, 100, 255);
        let b = Rgb::new(255, 100, 0);
        assert_eq!(a.mezclar(b, 0), a);
        assert_eq!(a.mezclar(b, 255), b);
        assert_eq!(a.mezclar(b, 128), Rgb::new(128, 100, 127));
    }

    #[test]
    fn acercar_no_se_pasa() {
        let a = Rgb::new(0, 250, 10);
        assert_eq!(a.acercar(Rgb::new(5, 255, 0), 20), Rgb::new(5, 255, 0));
        assert_eq!(a.acercar(Rgb::new(100, 0, 10), 20), Rgb::new(20, 230, 10));
    }

    #[test]
    fn gamma_monotona_y_extremos() {
        assert_eq!(gamma16(0), 0);
        assert_eq!(gamma16(255), 65535);
        assert!(gamma16(128) < 32768 / 2 + 1000); // mitad perceptual ≈ 25 % de potencia
        for c in 0..255u8 {
            assert!(gamma16(c) <= gamma16(c + 1));
        }
    }

    #[test]
    fn escalar() {
        assert_eq!(Rgb::new(200, 100, 0).escalar(128), Rgb::new(100, 50, 0));
        assert_eq!(Rgb::new(200, 100, 0).escalar(255), Rgb::new(200, 100, 0));
    }
}
