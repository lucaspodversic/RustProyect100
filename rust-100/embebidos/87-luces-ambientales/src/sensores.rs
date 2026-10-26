//! Sensores de entrada.
//!
//! `embedded-hal` 1.0 eliminó el trait de ADC (cada HAL lo expone distinto),
//! por eso definimos uno propio mínimo: [`SensorAnalogico`]. Para el PIR se
//! usa el estándar `embedded_hal::digital::InputPin`.

/// Fuente de una lectura analógica de 12 bits (0..=4095), como el ADC del
/// ESP32 o del RP2040.
pub trait SensorAnalogico {
    /// Error del periférico.
    type Error;
    /// Toma una muestra.
    fn leer(&mut self) -> Result<u16, Self::Error>;
}

/// Convierte la lectura de un divisor LDR (fotorresistor) a lux aproximados.
///
/// Usamos una tabla de calibración por tramos (más barata que el modelo
/// logarítmico real del LDR) e interpolamos linealmente entre puntos.
pub fn adc_a_lux(adc: u16) -> u32 {
    // (lectura ADC, lux) medidos con un luxómetro para un GL5528 + 10 kΩ.
    const TABLA: [(u16, u32); 6] = [
        (0, 0),
        (400, 5),
        (1200, 50),
        (2400, 300),
        (3500, 1500),
        (4095, 10000),
    ];
    let adc = adc.min(4095);
    for w in TABLA.windows(2) {
        let (x0, y0) = w[0];
        let (x1, y1) = w[1];
        if adc <= x1 {
            return y0 + (y1 - y0) * (adc - x0) as u32 / (x1 - x0) as u32;
        }
    }
    10000
}

/// Brillo (0..=255) que corresponde a una luz ambiente dada: a más lux,
/// menos hace falta iluminar. Por debajo de 10 lux → 100 %, por encima de
/// 1000 lux → apagado (es de día y entra sol).
pub fn brillo_por_lux(lux: u32) -> u8 {
    match lux {
        0..=10 => 255,
        l if l >= 1000 => 0,
        l => (255 - (l - 10) * 255 / 990) as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lux_interpola_y_satura() {
        assert_eq!(adc_a_lux(0), 0);
        assert_eq!(adc_a_lux(400), 5);
        assert_eq!(adc_a_lux(800), 27);
        assert_eq!(adc_a_lux(4095), 10000);
        assert_eq!(adc_a_lux(60000), 10000);
    }

    #[test]
    fn brillo_inverso_a_la_luz() {
        assert_eq!(brillo_por_lux(0), 255);
        assert_eq!(brillo_por_lux(5000), 0);
        let mut previo = 255;
        for lux in (10..1000).step_by(37) {
            let b = brillo_por_lux(lux);
            assert!(b <= previo);
            previo = b;
        }
    }
}
