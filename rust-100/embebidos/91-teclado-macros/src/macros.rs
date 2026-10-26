//! Macros: secuencias de pasos que se "teclean" a lo largo de varios ticks.
//!
//! Cada pulsación necesita **dos** reportes: uno con la tecla pulsada y otro
//! sin ella. Si se envían dos reportes iguales seguidos (por ejemplo "ll" en
//! "hola"), el host no ve que la tecla se soltó y escribe una sola "l". El
//! ejecutor intercala un reporte vacío después de cada carácter.

use crate::hid::{ascii_a_hid, Reporte};

/// Un paso de macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paso {
    /// Pulsa y suelta un código con modificadores.
    Toque(u8, u8),
    /// Escribe un texto ASCII (distribución US).
    Texto(&'static str),
    /// Espera `n` ticks (1 tick ≈ 1 ms de sondeo USB).
    Esperar(u16),
}

/// Ejecutor no bloqueante de una macro.
#[derive(Debug, Clone, Copy)]
pub struct Ejecutor {
    pasos: &'static [Paso],
    i: usize,
    /// Índice dentro de un `Texto`, o ticks restantes de `Esperar`.
    sub: usize,
    /// `true` si el próximo reporte debe ser el de "soltar".
    soltar: bool,
}

impl Ejecutor {
    /// Prepara la ejecución.
    pub const fn new(pasos: &'static [Paso]) -> Self {
        Self { pasos, i: 0, sub: 0, soltar: false }
    }

    /// ¿Terminó?
    pub fn terminado(&self) -> bool {
        self.i >= self.pasos.len() && !self.soltar
    }

    /// Devuelve el siguiente reporte, o `None` al terminar.
    pub fn siguiente(&mut self) -> Option<Reporte> {
        if self.soltar {
            self.soltar = false;
            return Some(Reporte::default());
        }
        loop {
            let paso = *self.pasos.get(self.i)?;
            match paso {
                Paso::Toque(c, m) => {
                    self.i += 1;
                    return Some(self.pulsar(c, m));
                }
                Paso::Texto(t) => {
                    // Se avanza por bytes; los caracteres no soportados se saltan.
                    while let Some(ch) = t[self.sub..].chars().next() {
                        self.sub += ch.len_utf8();
                        if let Some((c, m)) = ascii_a_hid(ch) {
                            return Some(self.pulsar(c, m));
                        }
                    }
                    self.i += 1;
                    self.sub = 0;
                }
                Paso::Esperar(n) => {
                    if self.sub < n as usize {
                        self.sub += 1;
                        return Some(Reporte::default());
                    }
                    self.i += 1;
                    self.sub = 0;
                }
            }
        }
    }

    fn pulsar(&mut self, codigo: u8, m: u8) -> Reporte {
        self.soltar = true;
        let mut r = Reporte { modificadores: m, ..Default::default() };
        r.agregar(codigo);
        r
    }
}

/// Macros por defecto (índice = número en `Accion::Macro`).
pub static MACROS: [&[Paso]; 2] = [
    // 0: firma de correo.
    &[Paso::Texto("Saludos,\nAna")],
    // 1: abrir el lanzador (GUI) y escribir "terminal" + Enter.
    &[
        Paso::Toque(0, crate::hid::mods::LGUI),
        Paso::Esperar(3),
        Paso::Texto("terminal\n"),
    ],
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::{kc, mods};

    #[test]
    fn texto_intercala_soltar() {
        static P: [Paso; 1] = [Paso::Texto("Hi")];
        let mut e = Ejecutor::new(&P);
        let r: Vec<_> = core::iter::from_fn(|| e.siguiente()).map(|r| r.bytes()).collect();
        assert_eq!(
            r,
            vec![
                [mods::LSHIFT, 0, 0x0B, 0, 0, 0, 0, 0],
                [0; 8],
                [0, 0, 0x0C, 0, 0, 0, 0, 0],
                [0; 8]
            ]
        );
        assert!(e.terminado());
    }

    #[test]
    fn esperar_y_toque() {
        static P: [Paso; 3] = [Paso::Toque(kc::A, 0), Paso::Esperar(2), Paso::Texto("ñb")];
        let mut e = Ejecutor::new(&P);
        let n = core::iter::from_fn(|| e.siguiente()).count();
        // A + soltar + 2 esperas + b + soltar ('ñ' se salta).
        assert_eq!(n, 6);
    }
}
