//! Composición de la pantalla según el estado del despertador.
//!
//! Layout (128×64):
//! ```text
//! ┌────────────────────────────┐
//! │ AJUSTE RELOJ        *      │ ← fila 0: título/estado (escala 1); '*' = alarma activa
//! │                            │
//! │       07:30                │ ← filas 16..37: hora grande (escala 3)
//! │                            │
//! │ ALARMA 06:45               │ ← fila 56: hora de alarma (escala 1)
//! └────────────────────────────┘
//! ```

use core::fmt::Write;
use embedded_hal::digital::OutputPin;

use crate::despertador::{Campo, Despertador, Estado};
use crate::hora::Hora;
use crate::pantalla::{Framebuffer, W};

/// Pequeño buffer de texto en la pila para formatear "HH:MM".
struct Txt {
    b: [u8; 24],
    n: usize,
}
impl Write for Txt {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        if self.n + bytes.len() > self.b.len() {
            return Err(core::fmt::Error);
        }
        self.b[self.n..self.n + bytes.len()].copy_from_slice(bytes);
        self.n += bytes.len();
        Ok(())
    }
}
impl Txt {
    fn hhmm(h: Hora) -> Txt {
        let mut t = Txt { b: [0; 24], n: 0 };
        let _ = write!(t, "{:02}:{:02}", h.h(), h.m());
        t
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.b[..self.n]).unwrap_or("")
    }
}

/// Dibuja la interfaz completa. `parpadeo` alterna cada medio segundo y se
/// usa para hacer titilar el campo en edición y los dos puntos.
pub fn dibujar<BZ: OutputPin>(fb: &mut Framebuffer, d: &Despertador<BZ>, parpadeo: bool) {
    fb.limpiar();
    let titulo = match d.estado() {
        Estado::Normal => "",
        Estado::AjusteReloj(_) => "AJUSTE RELOJ",
        Estado::AjusteAlarma(_) => "AJUSTE ALARMA",
        Estado::Sonando(_) => "DESPIERTA",
        Estado::Pospuesta(_) => "SNOOZE",
    };
    fb.texto(0, 0, titulo, 1);
    if d.alarma_activa {
        fb.texto(W as i32 - 6, 0, "*", 1);
    }
    // Hora grande centrada: en ajuste de alarma mostramos la alarma.
    let (hora_grande, campo) = match d.estado() {
        Estado::AjusteAlarma(c) => (d.alarma, Some(c)),
        Estado::AjusteReloj(c) => (d.ahora, Some(c)),
        _ => (d.ahora, None),
    };
    let mut txt = Txt::hhmm(hora_grande);
    if campo.is_none() && !parpadeo {
        // Dos puntos que titilan cada medio segundo, como un reloj clásico.
        txt.b[2] = b' ';
    }
    let escala = 3;
    let ancho = Framebuffer::ancho_texto(txt.as_str(), escala);
    let x0 = (W as i32 - ancho) / 2;
    fb.texto(x0, 16, txt.as_str(), escala);
    // Resaltado del campo en edición (invertido, titilando).
    if let (Some(c), true) = (campo, parpadeo) {
        let ancho_car = 6 * escala;
        let x = match c {
            Campo::Horas => x0,
            Campo::Minutos => x0 + 3 * ancho_car,
        };
        fb.invertir_rect(x - 1, 15, 2 * ancho_car, 7 * escala + 2);
    }
    // Línea inferior con la alarma.
    let alarma = Txt::hhmm(d.alarma);
    fb.texto(0, 56, "ALARMA", 1);
    fb.texto(42, 56, alarma.as_str(), 1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::botones::EventoBoton;
    use crate::despertador::Tecla;
    use embedded_hal_mock::eh1::digital::{Mock, State, Transaction};

    #[test]
    fn dibuja_hora_y_resaltado() {
        // El buzzer se pone en bajo en cada `tecla` (2 llamadas).
        let bz = Mock::new(&[Transaction::set(State::Low), Transaction::set(State::Low)]);
        let mut d = Despertador::new(Hora::hms(7, 30, 0), Hora::hms(6, 45, 0), bz);
        let mut fb = Framebuffer::default();
        dibujar(&mut fb, &d, true);
        // El '*' de alarma activa enciende algún píxel arriba a la derecha.
        assert!((122..128).any(|x| (0..7).any(|y| fb.get(x, y))));
        let normal = fb.bytes().iter().map(|b| b.count_ones()).sum::<u32>();
        d.tecla(Tecla::Modo, EventoBoton::Corto);
        dibujar(&mut fb, &d, true);
        let ajuste = fb.bytes().iter().map(|b| b.count_ones()).sum::<u32>();
        // El rectángulo invertido + título cambian mucho la cantidad de píxeles.
        assert!(ajuste > normal + 100, "{normal} vs {ajuste}");
        d.tecla(Tecla::Modo, EventoBoton::Corto);
        d.buzzer().done();
    }
}
