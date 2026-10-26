//! Acumulador estadístico de lecturas.
//!
//! En vez de guardar todas las muestras (no hay heap), mantenemos solo
//! **suma, cantidad, mínimo y máximo**. Así el costo en memoria es O(1) y el
//! promedio se calcula al publicar.

use crate::sht3x::Lectura;

/// Estadísticas de una magnitud entera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resumen {
    /// Valor mínimo observado.
    pub min: i32,
    /// Valor máximo observado.
    pub max: i32,
    /// Promedio redondeado al entero más cercano.
    pub promedio: i32,
}

/// Acumulador de una magnitud (temperatura o humedad).
#[derive(Debug, Clone, Copy, Default)]
pub struct Acumulador {
    suma: i64,
    cantidad: u32,
    min: i32,
    max: i32,
}

impl Acumulador {
    /// Agrega una muestra.
    pub fn agregar(&mut self, v: i32) {
        if self.cantidad == 0 {
            self.min = v;
            self.max = v;
        } else {
            self.min = self.min.min(v);
            self.max = self.max.max(v);
        }
        self.suma += v as i64;
        self.cantidad += 1;
    }

    /// Cantidad de muestras acumuladas.
    pub fn cantidad(&self) -> u32 {
        self.cantidad
    }

    /// Devuelve el resumen, o `None` si no hay muestras.
    pub fn resumen(&self) -> Option<Resumen> {
        if self.cantidad == 0 {
            return None;
        }
        let n = self.cantidad as i64;
        // Redondeo "half away from zero" en enteros: sumamos/restamos n/2.
        let promedio = if self.suma >= 0 {
            (self.suma + n / 2) / n
        } else {
            (self.suma - n / 2) / n
        };
        Some(Resumen {
            min: self.min,
            max: self.max,
            promedio: promedio as i32,
        })
    }

    /// Vuelve a cero (se llama tras cada publicación exitosa).
    pub fn reiniciar(&mut self) {
        *self = Self::default();
    }
}

/// Par de acumuladores para una estación de temperatura + humedad.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ventana {
    /// Temperatura (centésimas de °C).
    pub temperatura: Acumulador,
    /// Humedad (centésimas de %).
    pub humedad: Acumulador,
}

impl Ventana {
    /// Agrega una lectura completa.
    pub fn agregar(&mut self, l: &Lectura) {
        self.temperatura.agregar(l.temp_centi_c);
        self.humedad.agregar(l.humedad_centi_pct as i32);
    }
}

/// Punto de rocío aproximado (fórmula simplificada de Lawrence, válida para
/// HR > 50 %): `Td ≈ T − (100 − HR) / 5`. Entrada y salida en centésimas.
///
/// Se usa la versión lineal porque evita logaritmos (no hay `libm` aquí) y
/// su error es < 1 °C en el rango donde el rocío importa (HR alta).
pub fn punto_de_rocio_aprox(temp_centi: i32, hum_centi: u32) -> i32 {
    temp_centi - ((10000 - hum_centi as i32) / 5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vacio_no_tiene_resumen() {
        assert_eq!(Acumulador::default().resumen(), None);
    }

    #[test]
    fn min_max_promedio() {
        let mut a = Acumulador::default();
        for v in [10, -5, 30, 5] {
            a.agregar(v);
        }
        assert_eq!(
            a.resumen(),
            Some(Resumen { min: -5, max: 30, promedio: 10 })
        );
        a.reiniciar();
        assert_eq!(a.cantidad(), 0);
    }

    #[test]
    fn promedio_redondea_negativos() {
        let mut a = Acumulador::default();
        a.agregar(-1);
        a.agregar(-2); // -1.5 → -2
        assert_eq!(a.resumen().unwrap().promedio, -2);
    }

    #[test]
    fn rocio() {
        // 25 °C y 60 % → 25 − 40/5 = 17 °C.
        assert_eq!(punto_de_rocio_aprox(2500, 6000), 1700);
    }
}
