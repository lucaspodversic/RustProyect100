//! Máquina de estados del despertador.
//!
//! ```text
//!            MODO                MODO               MODO               MODO
//!  Normal ─────────► AjHora ─────────► AjMin ─────────► AlHora ─────────► AlMin ──MODO──► Normal
//!    │  ▲   (MÁS/MENOS cambian el campo resaltado en los modos de ajuste)
//!    │  │
//!    │  └──── MODO (apagar) ◄──── Sonando ──SNOOZE──► Pospuesta ──(5 min)──► Sonando
//!    │                              ▲
//!    └── llega la hora de alarma ───┘    (MODO largo en Normal: activa/desactiva alarma)
//! ```
//!
//! El buzzer suena con patrón 500 ms encendido / 500 ms apagado y se corta
//! solo a los 10 minutos para no sonar eternamente si no hay nadie.

use embedded_hal::digital::OutputPin;

use crate::botones::EventoBoton;
use crate::hora::Hora;

/// Duración de la posposición (snooze), en segundos.
pub const SNOOZE_S: u32 = 5 * 60;
/// Tiempo máximo sonando antes de auto‑apagarse, en ms.
pub const MAX_SONANDO_MS: u32 = 10 * 60 * 1000;

/// Campo que se está editando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo {
    /// Horas.
    Horas,
    /// Minutos.
    Minutos,
}

/// Estado de la máquina.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Mostrando la hora.
    Normal,
    /// Ajustando la hora del reloj.
    AjusteReloj(Campo),
    /// Ajustando la hora de la alarma.
    AjusteAlarma(Campo),
    /// Alarma sonando; guarda los ms que lleva sonando.
    Sonando(u32),
    /// Pospuesta hasta la hora indicada.
    Pospuesta(Hora),
}

/// Botones físicos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tecla {
    /// Botón MODO.
    Modo,
    /// Botón MÁS.
    Mas,
    /// Botón SNOOZE (también "menos" en ajuste).
    Snooze,
}

/// El despertador. Genérico sobre el pin del buzzer.
pub struct Despertador<BZ> {
    /// Hora actual.
    pub ahora: Hora,
    /// Hora de alarma.
    pub alarma: Hora,
    /// ¿Alarma habilitada?
    pub alarma_activa: bool,
    estado: Estado,
    ms_acumulados: u32,
    ultimo_minuto_disparado: Option<u32>,
    buzzer: BZ,
}

impl<BZ: OutputPin> Despertador<BZ> {
    /// Crea el despertador.
    pub fn new(ahora: Hora, alarma: Hora, buzzer: BZ) -> Self {
        Self {
            ahora,
            alarma,
            alarma_activa: true,
            estado: Estado::Normal,
            ms_acumulados: 0,
            ultimo_minuto_disparado: None,
            buzzer,
        }
    }

    /// Estado actual.
    pub fn estado(&self) -> Estado {
        self.estado
    }

    /// Acceso al buzzer (tests).
    pub fn buzzer(&mut self) -> &mut BZ {
        &mut self.buzzer
    }

    /// Avanza el tiempo `dt_ms` milisegundos.
    pub fn tick(&mut self, dt_ms: u32) {
        self.ms_acumulados += dt_ms;
        while self.ms_acumulados >= 1000 {
            self.ms_acumulados -= 1000;
            // En ajuste del reloj, el tiempo no avanza (como un reloj real).
            if !matches!(self.estado, Estado::AjusteReloj(_)) {
                self.ahora = self.ahora.sumar(1);
            }
        }
        match self.estado {
            Estado::Normal => {
                let minuto = self.ahora.total() / 60;
                // Disparamos una sola vez por minuto (si se apaga en el mismo
                // minuto, no vuelve a sonar).
                if self.alarma_activa
                    && self.ahora.mismo_minuto(self.alarma)
                    && self.ultimo_minuto_disparado != Some(minuto)
                {
                    self.ultimo_minuto_disparado = Some(minuto);
                    self.estado = Estado::Sonando(0);
                }
            }
            Estado::Sonando(ms) => {
                let ms = ms + dt_ms;
                self.estado = if ms >= MAX_SONANDO_MS { Estado::Normal } else { Estado::Sonando(ms) };
            }
            Estado::Pospuesta(hasta) => {
                if self.ahora >= hasta && self.ahora.total() - hasta.total() < 60 {
                    self.estado = Estado::Sonando(0);
                }
            }
            _ => {}
        }
        self.actualizar_buzzer();
    }

    /// Patrón del buzzer: encendido en la primera mitad de cada segundo.
    fn actualizar_buzzer(&mut self) {
        let encendido = matches!(self.estado, Estado::Sonando(ms) if (ms % 1000) < 500);
        let _ = if encendido { self.buzzer.set_high() } else { self.buzzer.set_low() };
    }

    /// Procesa un evento de botón.
    pub fn tecla(&mut self, t: Tecla, ev: EventoBoton) {
        use Campo::*;
        use Estado::*;
        let delta = match t {
            Tecla::Mas => 1,
            Tecla::Snooze => -1,
            Tecla::Modo => 0,
        };
        self.estado = match (self.estado, t, ev) {
            (Normal, Tecla::Modo, EventoBoton::Largo) => {
                self.alarma_activa = !self.alarma_activa;
                Normal
            }
            (Normal, Tecla::Modo, EventoBoton::Corto) => AjusteReloj(Horas),
            (AjusteReloj(Horas), Tecla::Modo, _) => AjusteReloj(Minutos),
            (AjusteReloj(Minutos), Tecla::Modo, _) => {
                // Al confirmar la hora se ponen los segundos en cero.
                self.ahora = Hora::hms(self.ahora.h(), self.ahora.m(), 0);
                self.ms_acumulados = 0;
                AjusteAlarma(Horas)
            }
            (AjusteAlarma(Horas), Tecla::Modo, _) => AjusteAlarma(Minutos),
            (AjusteAlarma(Minutos), Tecla::Modo, _) => Normal,
            (AjusteReloj(Horas), _, _) => {
                self.ahora = self.ahora.sumar_horas(delta);
                AjusteReloj(Horas)
            }
            (AjusteReloj(Minutos), _, _) => {
                self.ahora = self.ahora.sumar_minutos_ciclico(delta);
                AjusteReloj(Minutos)
            }
            (AjusteAlarma(Horas), _, _) => {
                self.alarma = self.alarma.sumar_horas(delta);
                AjusteAlarma(Horas)
            }
            (AjusteAlarma(Minutos), _, _) => {
                self.alarma = self.alarma.sumar_minutos_ciclico(delta);
                AjusteAlarma(Minutos)
            }
            (Sonando(_), Tecla::Snooze, _) => Pospuesta(self.ahora.sumar(SNOOZE_S as i64)),
            (Sonando(_), Tecla::Modo, _) | (Pospuesta(_), Tecla::Modo, _) => Normal,
            (e, _, _) => e,
        };
        self.actualizar_buzzer();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::Infallible;
    use embedded_hal::digital::ErrorType;
    use EventoBoton::*;

    /// Buzzer falso: recuerda el último nivel.
    #[derive(Default)]
    struct Buzzer(bool);
    impl ErrorType for Buzzer {
        type Error = Infallible;
    }
    impl OutputPin for Buzzer {
        fn set_high(&mut self) -> Result<(), Infallible> {
            self.0 = true;
            Ok(())
        }
        fn set_low(&mut self) -> Result<(), Infallible> {
            self.0 = false;
            Ok(())
        }
    }

    fn reloj(ahora: Hora, alarma: Hora) -> Despertador<Buzzer> {
        Despertador::new(ahora, alarma, Buzzer::default())
    }

    #[test]
    fn suena_a_la_hora_con_patron() {
        let mut d = reloj(Hora::hms(6, 59, 59), Hora::hms(7, 0, 0));
        d.tick(999);
        assert_eq!(d.estado(), Estado::Normal);
        d.tick(1);
        assert_eq!(d.estado(), Estado::Sonando(0));
        assert!(d.buzzer().0, "encendido al empezar");
        d.tick(600);
        assert!(!d.buzzer().0, "apagado en la segunda mitad");
    }

    #[test]
    fn snooze_y_vuelve_a_sonar() {
        let mut d = reloj(Hora::hms(7, 0, 0), Hora::hms(7, 0, 0));
        d.tick(10);
        d.tecla(Tecla::Snooze, Corto);
        assert_eq!(d.estado(), Estado::Pospuesta(Hora::hms(7, 5, 0)));
        assert!(!d.buzzer().0);
        for _ in 0..299 {
            d.tick(1000);
        }
        assert!(matches!(d.estado(), Estado::Pospuesta(_)));
        d.tick(1000);
        assert_eq!(d.estado(), Estado::Sonando(0));
    }

    #[test]
    fn apagar_no_redispara_en_el_mismo_minuto() {
        let mut d = reloj(Hora::hms(7, 0, 0), Hora::hms(7, 0, 0));
        d.tick(10);
        d.tecla(Tecla::Modo, Corto);
        assert_eq!(d.estado(), Estado::Normal);
        d.tick(1000);
        assert_eq!(d.estado(), Estado::Normal);
    }

    #[test]
    fn auto_apagado_a_los_10_minutos() {
        let mut d = reloj(Hora::hms(7, 0, 0), Hora::hms(7, 0, 0));
        d.tick(0);
        for _ in 0..600 {
            d.tick(1000);
        }
        assert_eq!(d.estado(), Estado::Normal);
        assert!(!d.buzzer().0);
    }

    #[test]
    fn ciclo_de_ajuste_completo() {
        let mut d = reloj(Hora::hms(10, 20, 33), Hora::hms(7, 0, 0));
        d.tecla(Tecla::Modo, Corto); // AjusteReloj(Horas)
        d.tecla(Tecla::Mas, Corto); // 11
        d.tecla(Tecla::Modo, Corto); // minutos
        d.tecla(Tecla::Snooze, Corto); // 19
        d.tick(5000); // el reloj no avanza durante el ajuste
        d.tecla(Tecla::Modo, Corto); // confirma → segundos en 0
        assert_eq!(d.ahora, Hora::hms(11, 19, 0));
        d.tecla(Tecla::Snooze, Corto); // alarma 06:00
        d.tecla(Tecla::Modo, Corto);
        d.tecla(Tecla::Mas, Corto); // 06:01
        d.tecla(Tecla::Modo, Corto);
        assert_eq!(d.alarma, Hora::hms(6, 1, 0));
        assert_eq!(d.estado(), Estado::Normal);
    }

    #[test]
    fn alarma_desactivada_no_suena() {
        let mut d = reloj(Hora::hms(6, 59, 59), Hora::hms(7, 0, 0));
        d.tecla(Tecla::Modo, Largo);
        assert!(!d.alarma_activa);
        d.tick(2000);
        assert_eq!(d.estado(), Estado::Normal);
    }
}
