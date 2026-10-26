//! Sensor capacitivo de humedad de suelo (tipo "Capacitive Soil Moisture v1.2").
//!
//! La salida analógica **baja** cuanto más húmedo está el suelo. Se calibra
//! midiendo el valor en aire (seco) y sumergido en agua (mojado), y se
//! interpola linealmente a porcentaje.

/// Trait de lectura analógica de 12 bits (embedded‑hal 1.0 no define ADC).
pub trait Adc {
    /// Lee el canal; `Err` si el periférico falla.
    fn leer(&mut self) -> Result<u16, ()>;
}

/// Calibración de un sensor.
#[derive(Debug, Clone, Copy)]
pub struct Calibracion {
    /// Lectura en aire (0 %).
    pub seco: u16,
    /// Lectura en agua (100 %).
    pub mojado: u16,
}

impl Default for Calibracion {
    fn default() -> Self {
        // Valores típicos con el ADC de 12 bits del ESP32/RP2040 a 3,3 V.
        Self { seco: 3000, mojado: 1300 }
    }
}

/// Resultado de una medición.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Humedad {
    /// Porcentaje 0..=100.
    Pct(u8),
    /// Lectura imposible (cable cortado → ~4095, o cortocircuito → ~0).
    Falla,
}

impl Calibracion {
    /// Convierte una lectura cruda a porcentaje. Se tolera un margen de 10 %
    /// fuera de la calibración (se satura); más allá se considera falla.
    pub fn a_porcentaje(&self, raw: u16) -> Humedad {
        let (seco, mojado) = (self.seco as i32, self.mojado as i32);
        let rango = seco - mojado;
        let margen = rango / 10;
        let r = raw as i32;
        if r > seco + margen || r < mojado - margen {
            return Humedad::Falla;
        }
        let pct = (seco - r) * 100 / rango;
        Humedad::Pct(pct.clamp(0, 100) as u8)
    }
}

/// Mediana de 5 lecturas: descarta picos (ruido eléctrico de la bomba,
/// por ejemplo) mejor que un promedio.
pub fn mediana5<A: Adc>(adc: &mut A) -> Result<u16, ()> {
    let mut m = [0u16; 5];
    for v in m.iter_mut() {
        *v = adc.leer()?;
    }
    // Ordenamiento por inserción: trivial para 5 elementos y sin alloc.
    for i in 1..m.len() {
        let mut j = i;
        while j > 0 && m[j - 1] > m[j] {
            m.swap(j - 1, j);
            j -= 1;
        }
    }
    Ok(m[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Secuencia(Vec<u16>);
    impl Adc for Secuencia {
        fn leer(&mut self) -> Result<u16, ()> {
            self.0.pop().ok_or(())
        }
    }

    #[test]
    fn calibracion_lineal_y_saturacion() {
        let c = Calibracion::default();
        assert_eq!(c.a_porcentaje(3000), Humedad::Pct(0));
        assert_eq!(c.a_porcentaje(1300), Humedad::Pct(100));
        assert_eq!(c.a_porcentaje(2150), Humedad::Pct(50));
        assert_eq!(c.a_porcentaje(3100), Humedad::Pct(0)); // dentro del margen
        assert_eq!(c.a_porcentaje(4095), Humedad::Falla); // cable cortado
        assert_eq!(c.a_porcentaje(0), Humedad::Falla); // cortocircuito
    }

    #[test]
    fn mediana_descarta_picos() {
        let mut a = Secuencia(vec![2000, 4095, 2010, 0, 1990]);
        assert_eq!(mediana5(&mut a), Ok(2000));
        let mut vacio = Secuencia(vec![1, 2]);
        assert_eq!(mediana5(&mut vacio), Err(()));
    }
}
