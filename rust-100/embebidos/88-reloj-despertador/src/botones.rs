//! Lectura de botones con **antirrebote** y detección de pulsación larga.
//!
//! Un pulsador mecánico "rebota": durante unos milisegundos alterna entre
//! abierto y cerrado. Usamos un *integrador*: un contador que sube mientras
//! el pin indica "presionado" y baja si no; solo cuando llega a los extremos
//! (0 o `UMBRAL`) cambiamos el estado lógico. Es robusto y no usa interrupciones.
//!
//! Los botones se cablean a GND con pull‑up interno → **activo en bajo**.

use embedded_hal::digital::InputPin;

/// Eventos que produce un botón.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventoBoton {
    /// Se soltó antes del umbral de pulsación larga.
    Corto,
    /// Se mantuvo presionado más de `ms_largo` (se emite una sola vez).
    Largo,
}

/// Botón con antirrebote.
pub struct Boton<P> {
    pin: P,
    integrador: u8,
    umbral: u8,
    presionado: bool,
    ms_presionado: u32,
    largo_emitido: bool,
    ms_largo: u32,
}

impl<P: InputPin> Boton<P> {
    /// `muestras_estables`: cuántas muestras seguidas hacen falta (con
    /// muestreo cada 5 ms, 4 muestras = 20 ms de antirrebote).
    pub fn new(pin: P, muestras_estables: u8, ms_largo: u32) -> Self {
        Self {
            pin,
            integrador: 0,
            umbral: muestras_estables.max(1),
            presionado: false,
            ms_presionado: 0,
            largo_emitido: false,
            ms_largo,
        }
    }

    /// Estado lógico actual (ya filtrado).
    pub fn presionado(&self) -> bool {
        self.presionado
    }

    /// Devuelve el pin (útil para `done()` en tests con mocks).
    pub fn liberar(self) -> P {
        self.pin
    }

    /// Muestrea el pin; `dt_ms` es el tiempo desde la muestra anterior.
    pub fn muestrear(&mut self, dt_ms: u32) -> Option<EventoBoton> {
        // Activo en bajo; si la lectura falla la ignoramos (no presionado).
        let crudo = self.pin.is_low().unwrap_or(false);
        if crudo {
            self.integrador = (self.integrador + 1).min(self.umbral);
        } else {
            self.integrador = self.integrador.saturating_sub(1);
        }
        let mut evento = None;
        if !self.presionado && self.integrador == self.umbral {
            // Flanco de bajada estable: empieza la pulsación.
            self.presionado = true;
            self.ms_presionado = 0;
            self.largo_emitido = false;
        } else if self.presionado && self.integrador == 0 {
            // Se soltó: si no fue larga, es corta.
            self.presionado = false;
            if !self.largo_emitido {
                evento = Some(EventoBoton::Corto);
            }
        } else if self.presionado {
            self.ms_presionado = self.ms_presionado.saturating_add(dt_ms);
            if !self.largo_emitido && self.ms_presionado >= self.ms_largo {
                self.largo_emitido = true;
                evento = Some(EventoBoton::Largo);
            }
        }
        evento
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::digital::{Mock, State, Transaction};

    fn secuencia(estados: &[State]) -> Mock {
        let tx: Vec<_> = estados.iter().map(|s| Transaction::get(*s)).collect();
        Mock::new(&tx)
    }

    #[test]
    fn rebote_no_genera_evento_y_pulsacion_estable_si() {
        use State::*;
        // Rebote (L H L H) y luego estable en bajo 3 muestras, después suelta.
        let pin = secuencia(&[Low, High, Low, High, Low, Low, Low, High, High, High]);
        let mut b = Boton::new(pin, 3, 1000);
        let mut eventos = vec![];
        for _ in 0..10 {
            if let Some(e) = b.muestrear(5) {
                eventos.push(e);
            }
        }
        assert_eq!(eventos, vec![EventoBoton::Corto]);
        b.liberar().done();
    }

    #[test]
    fn pulsacion_larga_una_sola_vez_y_sin_corto() {
        let mut estados = vec![State::Low; 30];
        estados.extend([State::High; 2]);
        let mut b = Boton::new(secuencia(&estados), 2, 100);
        let mut eventos = vec![];
        for _ in 0..32 {
            if let Some(e) = b.muestrear(10) {
                eventos.push(e);
            }
        }
        assert_eq!(eventos, vec![EventoBoton::Largo]);
        b.liberar().done();
    }
}
