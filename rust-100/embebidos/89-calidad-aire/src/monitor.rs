//! Integración: media móvil, AQI, alarma con histéresis y texto del LCD.
//!
//! ### Histéresis
//! La alarma se **enciende** cuando el AQI combinado supera `umbral_on` y se
//! **apaga** recién cuando baja de `umbral_off` (< `umbral_on`). Sin esto, un
//! valor que oscila 100↔101 haría prender y apagar el buzzer continuamente.

use core::fmt::Write;
use embedded_hal::digital::OutputPin;

use crate::aqi::{aqi_pm10, aqi_pm25, nivel_co2, Categoria, NivelCo2};
use crate::filtro::MediaMovil;
use crate::pms5003::Trama;

/// Estado calculado del aire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estado {
    /// PM2.5 promedio (µg/m³).
    pub pm25: u32,
    /// PM10 promedio (µg/m³).
    pub pm10: u32,
    /// AQI final = máximo de los sub‑índices (regla de la EPA).
    pub aqi: u16,
    /// Categoría.
    pub categoria: Categoria,
    /// Último CO₂ (si hay sensor).
    pub co2: Option<u16>,
    /// ¿Alarma activa?
    pub alarma: bool,
}

/// Monitor genérico sobre el pin de alarma (LED/buzzer).
pub struct Monitor<P> {
    pm25: MediaMovil<10>,
    pm10: MediaMovil<10>,
    co2: Option<u16>,
    alarma: bool,
    pin: P,
    /// AQI para encender.
    pub umbral_on: u16,
    /// AQI para apagar.
    pub umbral_off: u16,
}

impl<P: OutputPin> Monitor<P> {
    /// Crea el monitor con umbrales por defecto (alarma en "dañina para sensibles").
    pub fn new(pin: P) -> Self {
        Self {
            pm25: MediaMovil::default(),
            pm10: MediaMovil::default(),
            co2: None,
            alarma: false,
            pin,
            umbral_on: 101,
            umbral_off: 90,
        }
    }

    /// Registra una lectura de CO₂.
    pub fn co2(&mut self, ppm: u16) {
        self.co2 = Some(ppm);
    }

    /// Procesa una trama del PMS5003 y actualiza la alarma.
    pub fn procesar(&mut self, t: &Trama) -> Estado {
        let pm25 = self.pm25.agregar(t.pm2_5 as u32);
        let pm10 = self.pm10.agregar(t.pm10 as u32);
        let aqi = aqi_pm25(pm25 * 10).max(aqi_pm10(pm10 * 10));
        let co2_malo = matches!(self.co2.map(nivel_co2), Some(NivelCo2::Malo));
        // Histéresis.
        if aqi >= self.umbral_on || co2_malo {
            self.alarma = true;
        } else if aqi < self.umbral_off {
            self.alarma = false;
        }
        let _ = if self.alarma { self.pin.set_high() } else { self.pin.set_low() };
        Estado { pm25, pm10, aqi, categoria: Categoria::de_aqi(aqi), co2: self.co2, alarma: self.alarma }
    }

    /// Devuelve el pin.
    pub fn liberar(self) -> P {
        self.pin
    }
}

/// Escritor sobre un array de 16 bytes, rellenando con espacios.
struct Linea16 {
    b: [u8; 16],
    n: usize,
}
impl Write for Linea16 {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &c in s.as_bytes() {
            if self.n < 16 {
                self.b[self.n] = c;
                self.n += 1;
            } // lo que no entra se trunca: el LCD tiene 16 columnas fijas
        }
        Ok(())
    }
}

/// Arma las dos líneas del LCD:
/// ```text
/// PM2.5 12 AQI 56
/// CO2 650 MODERADA
/// ```
pub fn lineas_lcd(e: &Estado) -> ([u8; 16], [u8; 16]) {
    let mut l1 = Linea16 { b: [b' '; 16], n: 0 };
    let _ = write!(l1, "PM2.5 {} AQI {}", e.pm25, e.aqi);
    let mut l2 = Linea16 { b: [b' '; 16], n: 0 };
    match e.co2 {
        Some(c) => {
            let _ = write!(l2, "CO2 {} {}", c, e.categoria.corto());
        }
        None => {
            let _ = write!(l2, "{}", e.categoria.corto());
        }
    }
    if e.alarma {
        l2.b[15] = b'!';
    }
    (l1.b, l2.b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::digital::{Mock, State, Transaction};

    fn trama(pm25: u16) -> Trama {
        Trama { pm1_0: 0, pm2_5: pm25, pm10: pm25, particulas_0_3: 0 }
    }

    #[test]
    fn histeresis_de_alarma() {
        use State::*;
        let pin = Mock::new(&[
            Transaction::set(High),
            Transaction::set(High),
            Transaction::set(Low),
        ]);
        let mut m = Monitor::new(pin);
        // Promedio 40 µg/m³ → AQI ≈ 112 → enciende.
        let e = m.procesar(&trama(40)); // AQI ~ 112 → enciende
        assert!(e.alarma && e.aqi >= 101, "{e:?}");
        // Promedio (40+30)/2 = 35 → AQI 100: entre umbrales, sigue encendida.
        let e = m.procesar(&trama(30));
        assert!(e.alarma, "{e:?}");
        // Bajamos mucho: promedio (40+30+0)/3 = 23 → AQI 79 < 90 → apaga.
        let e = m.procesar(&trama(0));
        assert!(!e.alarma, "{e:?}");
        m.liberar().done();
    }

    #[test]
    fn co2_malo_dispara_alarma_y_lcd() {
        let pin = Mock::new(&[Transaction::set(State::High)]);
        let mut m = Monitor::new(pin);
        m.co2(1800);
        let e = m.procesar(&trama(5));
        assert!(e.alarma);
        let (l1, l2) = lineas_lcd(&e);
        assert_eq!(&l1, b"PM2.5 5 AQI 28  ");
        assert_eq!(&l2, b"CO2 1800 BUENA !");
        m.liberar().done();
    }
}
