//! Controlador principal.
//!
//! Se llama a [`Controlador::tick`] periódicamente (p. ej. cada 50 ms) con la
//! hora actual en minutos y el tiempo transcurrido. Cada tick:
//!
//! 1. Lee el PIR; si hay movimiento, reinicia el temporizador de ausencia.
//! 2. Lee el LDR y calcula el brillo según la luz ambiente.
//! 3. Calcula el **color objetivo** = curva(hora) × brillo (o negro si no hay
//!    nadie hace más de `apagado_tras_ms`, o el color manual si hay *override*).
//! 4. Acerca el color actual al objetivo `paso_fundido` unidades (fundido).
//! 5. Escribe los tres PWM con corrección gamma.

use embedded_hal::digital::InputPin;
use embedded_hal::pwm::SetDutyCycle;

use crate::color::{gamma16, Rgb};
use crate::horario::{color_para, Fotograma, CURVA_CIRCADIANA};
use crate::sensores::{adc_a_lux, brillo_por_lux, SensorAnalogico};

/// Parámetros configurables.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Milisegundos sin movimiento antes de apagar.
    pub apagado_tras_ms: u32,
    /// Máximo cambio por canal en cada tick (velocidad del fundido).
    pub paso_fundido: u8,
    /// Brillo mínimo cuando hay presencia (para que nunca quede a oscuras).
    pub brillo_minimo: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self { apagado_tras_ms: 5 * 60_000, paso_fundido: 4, brillo_minimo: 20 }
    }
}

/// Modo de funcionamiento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Automático (hora + sensores).
    Auto,
    /// Color fijo elegido por el usuario (desde una app o botón).
    Manual(Rgb),
    /// Apagado forzado.
    Apagado,
}

/// El controlador, genérico sobre los periféricos concretos.
pub struct Controlador<'c, R, G, B, PIR, LDR> {
    r: R,
    g: G,
    b: B,
    pir: PIR,
    ldr: LDR,
    curva: &'c [Fotograma],
    cfg: Config,
    modo: Modo,
    actual: Rgb,
    ms_sin_movimiento: u32,
}

impl<'c, R, G, B, PIR, LDR> Controlador<'c, R, G, B, PIR, LDR>
where
    R: SetDutyCycle,
    G: SetDutyCycle,
    B: SetDutyCycle,
    PIR: InputPin,
    LDR: SensorAnalogico,
{
    /// Crea el controlador con la curva circadiana por defecto.
    pub fn new(r: R, g: G, b: B, pir: PIR, ldr: LDR, cfg: Config) -> Self {
        Self {
            r,
            g,
            b,
            pir,
            ldr,
            curva: &CURVA_CIRCADIANA,
            cfg,
            modo: Modo::Auto,
            actual: Rgb::NEGRO,
            ms_sin_movimiento: 0,
        }
    }

    /// Reemplaza la curva horaria.
    pub fn con_curva(mut self, curva: &'c [Fotograma]) -> Self {
        self.curva = curva;
        self
    }

    /// Cambia el modo.
    pub fn set_modo(&mut self, m: Modo) {
        self.modo = m;
    }

    /// Color que se está mostrando ahora.
    pub fn actual(&self) -> Rgb {
        self.actual
    }

    /// Acceso a los PWM (para tests / depuración).
    pub fn pwm(&self) -> (&R, &G, &B) {
        (&self.r, &self.g, &self.b)
    }

    /// Calcula el color objetivo sin tocar el hardware de salida.
    pub fn objetivo(&mut self, minuto_dia: u16, dt_ms: u32) -> Rgb {
        // Presencia: si el PIR falla lo tratamos como "hay alguien" (falla
        // segura: mejor luz encendida de más que a oscuras).
        let movimiento = self.pir.is_high().unwrap_or(true);
        if movimiento {
            self.ms_sin_movimiento = 0;
        } else {
            self.ms_sin_movimiento = self.ms_sin_movimiento.saturating_add(dt_ms);
        }
        match self.modo {
            Modo::Apagado => Rgb::NEGRO,
            Modo::Manual(c) => c,
            Modo::Auto => {
                if self.ms_sin_movimiento >= self.cfg.apagado_tras_ms {
                    return Rgb::NEGRO;
                }
                // Si el LDR falla asumimos oscuridad (brillo completo).
                let lux = self.ldr.leer().map(adc_a_lux).unwrap_or(0);
                let brillo = brillo_por_lux(lux).max(self.cfg.brillo_minimo);
                color_para(self.curva, minuto_dia).escalar(brillo)
            }
        }
    }

    /// Paso completo: objetivo + fundido + escritura de PWM.
    pub fn tick(&mut self, minuto_dia: u16, dt_ms: u32) -> Result<Rgb, ErrorPwm> {
        let objetivo = self.objetivo(minuto_dia, dt_ms);
        self.actual = self.actual.acercar(objetivo, self.cfg.paso_fundido);
        escribir(&mut self.r, self.actual.r)?;
        escribir(&mut self.g, self.actual.g)?;
        escribir(&mut self.b, self.actual.b)?;
        Ok(self.actual)
    }
}

/// Error al escribir un canal PWM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorPwm;

/// Escribe un canal aplicando gamma: duty = gamma16(c) / 65535 del máximo.
fn escribir<P: SetDutyCycle>(p: &mut P, c: u8) -> Result<(), ErrorPwm> {
    p.set_duty_cycle_fraction(gamma16(c), u16::MAX).map_err(|_| ErrorPwm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::Infallible;
    use embedded_hal::digital::ErrorType as PinErrorType;
    use embedded_hal::pwm::ErrorType as PwmErrorType;
    use std::cell::Cell;
    use std::rc::Rc;

    /// PWM falso con resolución de 16 bits que guarda el último duty.
    #[derive(Default)]
    struct PwmFalso {
        duty: u16,
    }
    impl PwmErrorType for PwmFalso {
        type Error = Infallible;
    }
    impl SetDutyCycle for PwmFalso {
        fn max_duty_cycle(&self) -> u16 {
            u16::MAX
        }
        fn set_duty_cycle(&mut self, d: u16) -> Result<(), Infallible> {
            self.duty = d;
            Ok(())
        }
    }

    /// PIR compartido: el test cambia `Cell` para simular movimiento.
    struct PirFalso(Rc<Cell<bool>>);
    impl PinErrorType for PirFalso {
        type Error = Infallible;
    }
    impl InputPin for PirFalso {
        fn is_high(&mut self) -> Result<bool, Infallible> {
            Ok(self.0.get())
        }
        fn is_low(&mut self) -> Result<bool, Infallible> {
            Ok(!self.0.get())
        }
    }

    struct LdrFalso(Rc<Cell<u16>>);
    impl SensorAnalogico for LdrFalso {
        type Error = ();
        fn leer(&mut self) -> Result<u16, ()> {
            Ok(self.0.get())
        }
    }

    type Ctl = Controlador<'static, PwmFalso, PwmFalso, PwmFalso, PirFalso, LdrFalso>;

    fn armar(cfg: Config) -> (Ctl, Rc<Cell<bool>>, Rc<Cell<u16>>) {
        let pir = Rc::new(Cell::new(true));
        let ldr = Rc::new(Cell::new(0));
        let c = Controlador::new(
            PwmFalso::default(),
            PwmFalso::default(),
            PwmFalso::default(),
            PirFalso(pir.clone()),
            LdrFalso(ldr.clone()),
            cfg,
        );
        (c, pir, ldr)
    }

    #[test]
    fn noche_oscura_da_rojo_nocturno_completo() {
        let (mut c, _, _) = armar(Config { paso_fundido: 255, ..Default::default() });
        let color = c.tick(22 * 60, 50).unwrap();
        assert_eq!(color, Rgb::new(120, 20, 0));
        // PWM con gamma: 120 → 120²·65535/65025.
        assert_eq!(c.pwm().0.duty, gamma16(120));
        assert_eq!(c.pwm().2.duty, 0);
    }

    #[test]
    fn fundido_gradual() {
        let (mut c, _, _) = armar(Config { paso_fundido: 10, ..Default::default() });
        let primero = c.tick(13 * 60, 50).unwrap();
        assert_eq!(primero, Rgb::new(10, 10, 10));
        for _ in 0..100 {
            c.tick(13 * 60, 50).unwrap();
        }
        assert_eq!(c.actual(), Rgb::new(240, 245, 255));
    }

    #[test]
    fn mucha_luz_baja_al_minimo() {
        let (mut c, _, ldr) = armar(Config { paso_fundido: 255, brillo_minimo: 20, ..Default::default() });
        ldr.set(4095); // pleno sol
        let color = c.tick(13 * 60, 50).unwrap();
        assert_eq!(color, Rgb::new(240, 245, 255).escalar(20));
    }

    #[test]
    fn se_apaga_sin_presencia_y_vuelve() {
        let cfg = Config { apagado_tras_ms: 1000, paso_fundido: 255, brillo_minimo: 0 };
        let (mut c, pir, _) = armar(cfg);
        assert_ne!(c.tick(12 * 60, 100).unwrap(), Rgb::NEGRO);
        pir.set(false);
        for _ in 0..9 {
            assert_ne!(c.tick(12 * 60, 100).unwrap(), Rgb::NEGRO);
        }
        assert_eq!(c.tick(12 * 60, 100).unwrap(), Rgb::NEGRO); // 1000 ms
        pir.set(true);
        assert_ne!(c.tick(12 * 60, 100).unwrap(), Rgb::NEGRO);
    }

    #[test]
    fn modos_manual_y_apagado() {
        let (mut c, _, _) = armar(Config { paso_fundido: 255, ..Default::default() });
        c.set_modo(Modo::Manual(Rgb::new(1, 2, 3)));
        assert_eq!(c.tick(0, 10).unwrap(), Rgb::new(1, 2, 3));
        c.set_modo(Modo::Apagado);
        assert_eq!(c.tick(0, 10).unwrap(), Rgb::NEGRO);
    }

    #[test]
    fn pir_con_embedded_hal_mock() {
        // Mismo flujo pero con el pin mock oficial, que verifica las lecturas.
        use embedded_hal_mock::eh1::digital::{Mock, State, Transaction};
        let mut pir = Mock::new(&[Transaction::get(State::Low)]);
        let ldr = Rc::new(Cell::new(0));
        let cfg = Config { apagado_tras_ms: 10, paso_fundido: 255, brillo_minimo: 0 };
        let mut c = Controlador::new(
            PwmFalso::default(),
            PwmFalso::default(),
            PwmFalso::default(),
            pir.clone(),
            LdrFalso(ldr),
            cfg,
        );
        assert_eq!(c.tick(600, 10).unwrap(), Rgb::NEGRO);
        pir.done();
    }
}
