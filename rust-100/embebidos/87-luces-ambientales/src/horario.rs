//! Curva de color según la hora del día.
//!
//! Se define una lista de **fotogramas clave** `(minuto_del_día, color)`
//! ordenada. Para un minuto cualquiera buscamos los dos fotogramas que lo
//! rodean e interpolamos linealmente. La curva es **circular**: después del
//! último fotograma se interpola hacia el primero del día siguiente.

use crate::color::Rgb;

/// Un fotograma clave de la curva.
#[derive(Debug, Clone, Copy)]
pub struct Fotograma {
    /// Minuto del día (0..1440).
    pub minuto: u16,
    /// Color en ese instante.
    pub color: Rgb,
}

/// Curva por defecto inspirada en la temperatura de color de la luz solar.
pub const CURVA_CIRCADIANA: [Fotograma; 6] = [
    Fotograma { minuto: 0, color: Rgb::new(40, 4, 0) },          // 00:00 rojo muy tenue
    Fotograma { minuto: 6 * 60, color: Rgb::new(255, 120, 40) }, // 06:00 amanecer cálido
    Fotograma { minuto: 9 * 60, color: Rgb::new(255, 220, 180) },// 09:00 mañana
    Fotograma { minuto: 13 * 60, color: Rgb::new(240, 245, 255) },// 13:00 blanco frío
    Fotograma { minuto: 19 * 60, color: Rgb::new(255, 140, 50) },// 19:00 atardecer ámbar
    Fotograma { minuto: 22 * 60, color: Rgb::new(120, 20, 0) },  // 22:00 rojo nocturno
];

/// Minutos que tiene un día.
pub const MINUTOS_DIA: u16 = 1440;

/// Devuelve el color para `minuto` (se toma módulo 1440) según `curva`.
///
/// Precondición: `curva` no vacía y ordenada por minuto ascendente.
pub fn color_para(curva: &[Fotograma], minuto: u16) -> Rgb {
    let m = minuto % MINUTOS_DIA;
    let n = curva.len();
    // Buscamos el último fotograma con minuto <= m (si no hay, es el último
    // del día anterior → índice n-1).
    let idx = curva.iter().rposition(|f| f.minuto <= m).unwrap_or(n - 1);
    let a = curva[idx];
    let b = curva[(idx + 1) % n];
    // Distancias circulares (sumando un día si damos la vuelta).
    let dist = |desde: u16, hasta: u16| (hasta + MINUTOS_DIA - desde) % MINUTOS_DIA;
    let total = dist(a.minuto, b.minuto).max(1) as u32;
    let recorrido = dist(a.minuto, m) as u32;
    let t = (recorrido * 255 / total) as u8;
    a.color.mezclar(b.color, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exacto_en_fotogramas() {
        for f in CURVA_CIRCADIANA {
            assert_eq!(color_para(&CURVA_CIRCADIANA, f.minuto), f.color);
        }
    }

    #[test]
    fn interpola_entre_fotogramas() {
        // 07:30 está a mitad de camino entre 06:00 y 09:00.
        let c = color_para(&CURVA_CIRCADIANA, 7 * 60 + 30);
        assert_eq!(c.r, 255);
        assert!(c.g > 160 && c.g < 180, "{c:?}");
    }

    #[test]
    fn da_la_vuelta_a_medianoche() {
        // 23:00 entre 22:00 (120,20,0) y 00:00 (40,4,0) → a mitad.
        let c = color_para(&CURVA_CIRCADIANA, 23 * 60);
        assert_eq!(c, Rgb::new(80, 12, 0));
        // 1440 ≡ 0.
        assert_eq!(color_para(&CURVA_CIRCADIANA, 1440), CURVA_CIRCADIANA[0].color);
    }

    #[test]
    fn curva_de_un_solo_fotograma() {
        let c = [Fotograma { minuto: 300, color: Rgb::new(1, 2, 3) }];
        assert_eq!(color_para(&c, 0), Rgb::new(1, 2, 3));
        assert_eq!(color_para(&c, 900), Rgb::new(1, 2, 3));
    }
}
