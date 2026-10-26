//! Framebuffer monocromo y driver del controlador **SSD1306**.
//!
//! ## Memoria del SSD1306
//! La GDDRAM de 128×64 está organizada en **8 páginas** de 8 filas. Cada
//! byte representa una columna de 8 píxeles verticales (bit 0 arriba). Por
//! eso el framebuffer usa ese mismo layout: `buf[pagina * 128 + x]`, y el
//! volcado a la pantalla es una copia directa.
//!
//! ## Protocolo I2C
//! Cada escritura empieza con un *byte de control*: `0x00` = siguen comandos,
//! `0x40` = siguen datos de pantalla.

use embedded_hal::i2c::I2c;

use crate::fuente::{glifo, ALTO, ANCHO};

/// Ancho en píxeles.
pub const W: usize = 128;
/// Alto en píxeles.
pub const H: usize = 64;
/// Tamaño del framebuffer en bytes (1 bit por píxel).
pub const BYTES: usize = W * H / 8;

/// Framebuffer 1 bpp en el layout por páginas del SSD1306.
pub struct Framebuffer {
    buf: [u8; BYTES],
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self { buf: [0; BYTES] }
    }
}

impl Framebuffer {
    /// Borra todo.
    pub fn limpiar(&mut self) {
        self.buf = [0; BYTES];
    }

    /// Enciende/apaga un píxel (fuera de rango se ignora: recorte implícito).
    pub fn pixel(&mut self, x: i32, y: i32, encendido: bool) {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        let idx = (y / 8) * W + x;
        let mascara = 1 << (y % 8);
        if encendido {
            self.buf[idx] |= mascara;
        } else {
            self.buf[idx] &= !mascara;
        }
    }

    /// Lee un píxel.
    pub fn get(&self, x: usize, y: usize) -> bool {
        x < W && y < H && self.buf[(y / 8) * W + x] & (1 << (y % 8)) != 0
    }

    /// Dibuja un carácter escalado `escala` veces en (x, y). Devuelve el
    /// avance horizontal (ancho + 1 columna de separación).
    pub fn caracter(&mut self, x: i32, y: i32, c: char, escala: i32) -> i32 {
        if let Some(g) = glifo(c) {
            for (col, bits) in g.iter().enumerate() {
                for fila in 0..ALTO {
                    if bits & (1 << fila) != 0 {
                        // Cada píxel del glifo se vuelve un bloque escala×escala.
                        for dx in 0..escala {
                            for dy in 0..escala {
                                self.pixel(x + col as i32 * escala + dx, y + fila as i32 * escala + dy, true);
                            }
                        }
                    }
                }
            }
        }
        (ANCHO as i32 + 1) * escala
    }

    /// Dibuja un texto.
    pub fn texto(&mut self, x: i32, y: i32, s: &str, escala: i32) {
        let mut cx = x;
        for c in s.chars() {
            cx += self.caracter(cx, y, c, escala);
        }
    }

    /// Ancho en píxeles que ocupará un texto (para centrarlo).
    pub fn ancho_texto(s: &str, escala: i32) -> i32 {
        s.chars().count() as i32 * (ANCHO as i32 + 1) * escala - escala
    }

    /// Invierte un rectángulo (para resaltar el campo en edición).
    pub fn invertir_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for yy in y..y + h {
            for xx in x..x + w {
                if xx >= 0 && yy >= 0 && (xx as usize) < W && (yy as usize) < H {
                    let v = self.get(xx as usize, yy as usize);
                    self.pixel(xx, yy, !v);
                }
            }
        }
    }

    /// Bytes crudos (para el volcado).
    pub fn bytes(&self) -> &[u8; BYTES] {
        &self.buf
    }

    /// Representación ASCII (solo en tests/host) para depurar visualmente.
    #[cfg(test)]
    pub fn ascii(&self, x0: usize, y0: usize, w: usize, h: usize) -> String {
        let mut s = String::new();
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                s.push(if self.get(x, y) { '#' } else { '.' });
            }
            s.push('\n');
        }
        s
    }
}

/// Dirección I2C típica de los módulos SSD1306 (0x3D si SA0 a VCC).
pub const DIRECCION_SSD1306: u8 = 0x3C;

/// Secuencia de inicialización para un panel 128×64 con charge pump interno.
pub const INIT: [u8; 25] = [
    0xAE, // display OFF
    0xD5, 0x80, // divisor del reloj
    0xA8, 0x3F, // multiplex = 64 filas
    0xD3, 0x00, // offset vertical 0
    0x40, // línea de inicio 0
    0x8D, 0x14, // charge pump ON
    0x20, 0x00, // direccionamiento horizontal
    0xA1, // segment remap (espejo horizontal)
    0xC8, // COM scan descendente (espejo vertical)
    0xDA, 0x12, // configuración de pines COM
    0x81, 0xCF, // contraste
    0xD9, 0xF1, // pre‑charge
    0xDB, 0x40, // VCOMH
    0xA4, // mostrar RAM
    0xA6, // modo normal (no invertido)
    0xAF, // display ON
];

/// Bytes de datos por transacción al volcar (los buffers de I2C de varios
/// HAL tienen límites; 16 + 1 de control es seguro en todos).
pub const BLOQUE: usize = 16;

/// Driver SSD1306 sobre I2C.
pub struct Ssd1306<I2C> {
    i2c: I2C,
    dir: u8,
}

impl<I2C: I2c> Ssd1306<I2C> {
    /// Crea el driver.
    pub fn new(i2c: I2C, dir: u8) -> Self {
        Self { i2c, dir }
    }

    /// Devuelve el bus.
    pub fn liberar(self) -> I2C {
        self.i2c
    }

    /// Envía la secuencia de arranque.
    pub fn iniciar(&mut self) -> Result<(), I2C::Error> {
        let mut cmd = [0u8; INIT.len() + 1];
        cmd[0] = 0x00; // byte de control: comandos
        cmd[1..].copy_from_slice(&INIT);
        self.i2c.write(self.dir, &cmd)
    }

    /// Ajusta el contraste (brillo) 0..=255; útil para un modo nocturno.
    pub fn contraste(&mut self, v: u8) -> Result<(), I2C::Error> {
        self.i2c.write(self.dir, &[0x00, 0x81, v])
    }

    /// Vuelca el framebuffer completo.
    pub fn volcar(&mut self, fb: &Framebuffer) -> Result<(), I2C::Error> {
        // Ventana completa: columnas 0..127, páginas 0..7.
        self.i2c.write(self.dir, &[0x00, 0x21, 0, 127, 0x22, 0, 7])?;
        let mut paquete = [0u8; BLOQUE + 1];
        paquete[0] = 0x40; // byte de control: datos
        for trozo in fb.bytes().chunks(BLOQUE) {
            paquete[1..].copy_from_slice(trozo);
            self.i2c.write(self.dir, &paquete)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::i2c::{Mock, Transaction};

    #[test]
    fn pixeles_y_paginas() {
        let mut fb = Framebuffer::default();
        fb.pixel(3, 9, true); // página 1, bit 1
        assert_eq!(fb.bytes()[128 + 3], 0b10);
        assert!(fb.get(3, 9));
        fb.pixel(3, 9, false);
        assert!(!fb.get(3, 9));
        fb.pixel(-1, 500, true); // fuera de rango: no explota
    }

    #[test]
    fn dibuja_un_uno() {
        let mut fb = Framebuffer::default();
        fb.texto(0, 0, "1", 1);
        let arte = fb.ascii(0, 0, 5, 7);
        assert_eq!(arte, "..#..\n.##..\n..#..\n..#..\n..#..\n..#..\n.###.\n");
    }

    #[test]
    fn escala_duplica() {
        let mut fb = Framebuffer::default();
        fb.texto(0, 0, "1", 2);
        assert!(fb.get(4, 0) && fb.get(5, 1) && !fb.get(3, 0));
        assert_eq!(Framebuffer::ancho_texto("12:00", 3), 5 * 6 * 3 - 3);
    }

    #[test]
    fn init_y_volcado_por_i2c() {
        let mut tx = vec![Transaction::write(0x3C, [&[0x00u8][..], &INIT].concat())];
        tx.push(Transaction::write(0x3C, vec![0x00, 0x21, 0, 127, 0x22, 0, 7]));
        let mut fb = Framebuffer::default();
        fb.pixel(0, 0, true);
        for i in 0..BYTES / BLOQUE {
            let mut p = vec![0x40];
            p.extend_from_slice(&fb.bytes()[i * BLOQUE..(i + 1) * BLOQUE]);
            tx.push(Transaction::write(0x3C, p));
        }
        let mut oled = Ssd1306::new(Mock::new(&tx), DIRECCION_SSD1306);
        oled.iniciar().unwrap();
        oled.volcar(&fb).unwrap();
        oled.liberar().done();
    }
}
