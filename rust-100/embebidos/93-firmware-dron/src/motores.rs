//! Salida a los ESC (variadores) por PWM con `SetDutyCycle`.
//!
//! Los ESC clásicos interpretan un pulso de **1000 µs = parado** a
//! **2000 µs = máximo**. Con PWM a 50 Hz (periodo de 20 ms), eso equivale a
//! un ciclo de trabajo del 5 % al 10 %. Los ESC modernos aceptan 400 Hz
//! (OneShot) o protocolos digitales (DShot). Aquí se usa el PWM estándar,
//! que es el más compatible, y la frecuencia se puede configurar.

use embedded_hal::pwm::SetDutyCycle;

/// Pulso mínimo (motor parado), µs.
pub const PULSO_MIN_US: u32 = 1000;
/// Pulso máximo, µs.
pub const PULSO_MAX_US: u32 = 2000;
/// Empuje mínimo con el dron armado (*idle*): los motores giran despacio
/// para responder rápido y no perder autoridad de control.
pub const RALENTI: f32 = 0.05;

/// Los 4 canales PWM.
pub struct Motores<P> {
    canales: [P; 4],
    periodo_us: u32,
}

impl<P: SetDutyCycle> Motores<P> {
    /// Crea la salida y manda el pulso mínimo (los ESC necesitan verlo al
    /// encender para "armarse" y no arrancar).
    pub fn new(canales: [P; 4], frecuencia_hz: u32) -> Self {
        let mut m = Self { canales, periodo_us: 1_000_000 / frecuencia_hz };
        m.parar();
        m
    }

    fn pulso(&mut self, i: usize, us: u32) {
        let max = self.canales[i].max_duty_cycle() as u32;
        let duty = (us as u64 * max as u64 / self.periodo_us as u64) as u16;
        let _ = self.canales[i].set_duty_cycle(duty);
    }

    /// Envía los empujes normalizados `0..=1`.
    pub fn escribir(&mut self, empuje: [f32; 4]) {
        for (i, e) in empuje.iter().enumerate() {
            let us = PULSO_MIN_US + (e.clamp(0.0, 1.0) * (PULSO_MAX_US - PULSO_MIN_US) as f32) as u32;
            self.pulso(i, us);
        }
    }

    /// Para todos los motores (pulso mínimo).
    pub fn parar(&mut self) {
        for i in 0..4 {
            self.pulso(i, PULSO_MIN_US);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::pwm::{Mock, Transaction as T};

    #[test]
    fn pulsos_a_duty_50hz() {
        // max_duty = 20000 → 1 cuenta = 1 µs a 50 Hz.
        let canal = |final_us: u16| {
            Mock::new(&[
                T::max_duty_cycle(20000),
                T::set_duty_cycle(1000),
                T::max_duty_cycle(20000),
                T::set_duty_cycle(final_us),
            ])
        };
        let mut cs = [canal(1000), canal(1500), canal(2000), canal(1250)];
        let mut m = Motores::new(cs.clone(), 50);
        m.escribir([0.0, 0.5, 1.2, 0.25]);
        for c in cs.iter_mut() {
            c.done();
        }
    }
}
