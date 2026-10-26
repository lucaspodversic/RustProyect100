//! Índice de calidad del aire (AQI) de la **EPA de EE.UU., revisión 2024**.
//!
//! Para cada contaminante hay una tabla de cortes `[C_lo, C_hi] → [I_lo, I_hi]`
//! y el índice se interpola linealmente:
//!
//! ```text
//! I = (I_hi − I_lo) / (C_hi − C_lo) · (C − C_lo) + I_lo
//! ```
//!
//! Las concentraciones se manejan en **décimas de µg/m³** para respetar el
//! truncado a 1 decimal que pide la EPA sin usar punto flotante.

/// Categorías oficiales (con nombre en castellano).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Categoria {
    /// 0–50.
    Buena,
    /// 51–100.
    Moderada,
    /// 101–150.
    DaninaSensibles,
    /// 151–200.
    Danina,
    /// 201–300.
    MuyDanina,
    /// 301+.
    Peligrosa,
}

impl Categoria {
    /// A partir del valor AQI.
    pub fn de_aqi(aqi: u16) -> Self {
        match aqi {
            0..=50 => Self::Buena,
            51..=100 => Self::Moderada,
            101..=150 => Self::DaninaSensibles,
            151..=200 => Self::Danina,
            201..=300 => Self::MuyDanina,
            _ => Self::Peligrosa,
        }
    }
    /// Texto corto (cabe en el LCD de 16 columnas).
    pub fn corto(self) -> &'static str {
        match self {
            Self::Buena => "BUENA",
            Self::Moderada => "MODERADA",
            Self::DaninaSensibles => "SENSIBLES",
            Self::Danina => "DANINA",
            Self::MuyDanina => "MUY DANINA",
            Self::Peligrosa => "PELIGRO",
        }
    }
}

/// Fila de la tabla: (C_lo, C_hi en décimas de µg/m³, I_lo, I_hi).
type Corte = (u32, u32, u16, u16);

/// PM2.5 (24 h), revisión de febrero de 2024.
const PM25: [Corte; 6] = [
    (0, 90, 0, 50),
    (91, 354, 51, 100),
    (355, 554, 101, 150),
    (555, 1254, 151, 200),
    (1255, 2254, 201, 300),
    (2255, 3254, 301, 500),
];

/// PM10 (24 h): la EPA la expresa en enteros; multiplicamos por 10.
const PM10: [Corte; 6] = [
    (0, 540, 0, 50),
    (550, 1540, 51, 100),
    (1550, 2540, 101, 150),
    (2550, 3540, 151, 200),
    (3550, 4240, 201, 300),
    (4250, 6040, 301, 500),
];

fn interpolar(tabla: &[Corte], c_decimas: u32) -> u16 {
    for &(c_lo, c_hi, i_lo, i_hi) in tabla {
        if c_decimas <= c_hi {
            // Si cae en el "hueco" entre filas (p. ej. 9,05 truncado), usamos la fila actual.
            let c = c_decimas.max(c_lo);
            let num = (i_hi - i_lo) as u32 * (c - c_lo);
            let den = c_hi - c_lo;
            // Redondeo al entero más cercano como indica la EPA.
            return i_lo + ((num + den / 2) / den) as u16;
        }
    }
    500 // fuera de escala
}

/// AQI de PM2.5 a partir de µg/m³ en décimas (125 = 12,5 µg/m³).
pub fn aqi_pm25(decimas: u32) -> u16 {
    interpolar(&PM25, decimas)
}

/// AQI de PM10 a partir de µg/m³ en décimas.
pub fn aqi_pm10(decimas: u32) -> u16 {
    interpolar(&PM10, decimas)
}

/// Nivel de CO₂ en interiores (no es parte del AQI; guía de ventilación).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NivelCo2 {
    /// < 800 ppm: aire exterior / muy bien ventilado.
    Excelente,
    /// 800–999 ppm.
    Aceptable,
    /// 1000–1499 ppm: conviene ventilar.
    Ventilar,
    /// ≥ 1500 ppm: somnolencia y dolor de cabeza.
    Malo,
}

/// Clasifica el CO₂.
pub fn nivel_co2(ppm: u16) -> NivelCo2 {
    match ppm {
        0..=799 => NivelCo2::Excelente,
        800..=999 => NivelCo2::Aceptable,
        1000..=1499 => NivelCo2::Ventilar,
        _ => NivelCo2::Malo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pm25_cortes_oficiales() {
        assert_eq!(aqi_pm25(0), 0);
        assert_eq!(aqi_pm25(90), 50);
        assert_eq!(aqi_pm25(91), 51);
        assert_eq!(aqi_pm25(354), 100);
        assert_eq!(aqi_pm25(555), 151);
        assert_eq!(aqi_pm25(5000), 500);
        // 12,0 µg/m³: 51 + 49·(120−91)/(354−91) = 56,4 → 56
        assert_eq!(aqi_pm25(120), 56);
    }

    #[test]
    fn pm10_y_categorias() {
        assert_eq!(aqi_pm10(540), 50);
        assert_eq!(aqi_pm10(1000), 73);
        assert_eq!(Categoria::de_aqi(56), Categoria::Moderada);
        assert_eq!(Categoria::de_aqi(301).corto(), "PELIGRO");
    }

    #[test]
    fn co2() {
        assert_eq!(nivel_co2(420), NivelCo2::Excelente);
        assert_eq!(nivel_co2(1200), NivelCo2::Ventilar);
        assert_eq!(nivel_co2(2500), NivelCo2::Malo);
    }
}
