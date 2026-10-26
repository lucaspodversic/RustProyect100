//! Lógica pura de agenda (sin I/O → fácil de testear).
//!
//! Representamos los instantes como **minutos desde 1970-01-01 00:00** en una
//! zona horaria fija (la del negocio). Así comparar, sumar y alinear es
//! aritmética entera. Los intervalos son **semiabiertos** `[inicio, fin)`:
//! una reserva 10:00–11:00 NO choca con otra 11:00–12:00.

pub const MIN_DIA: i64 = 24 * 60;

/// Días desde 1970-01-01 para una fecha civil (algoritmo de H. Hinnant).
fn dias_desde_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inversa de [`dias_desde_civil`].
fn civil_desde_dias(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn dias_del_mes(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// `"2026-10-26"` → minuto de las 00:00 de ese día. Valida la fecha real (no 31/02).
pub fn parse_fecha(s: &str) -> Option<i64> {
    let p: Vec<&str> = s.split('-').collect();
    if p.len() != 3 || p[0].len() != 4 || p[1].len() != 2 || p[2].len() != 2 {
        return None;
    }
    let (y, m, d) = (p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?);
    if !(1..=12).contains(&m) || d < 1 || d > dias_del_mes(y, m) {
        return None;
    }
    Some(dias_desde_civil(y, m, d) * MIN_DIA)
}

/// `"09:30"` → 570 (minutos desde medianoche). Acepta `"24:00"` como fin de día.
pub fn parse_hora(s: &str) -> Option<i64> {
    let (h, m) = s.split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
    if m >= 60 || h > 24 || (h == 24 && m != 0) {
        return None;
    }
    Some(h * 60 + m)
}

/// `"2026-10-26T09:30"` → minutos absolutos.
pub fn parse_fecha_hora(s: &str) -> Option<i64> {
    let (f, h) = s.split_once('T')?;
    let h = parse_hora(h)?;
    (h < MIN_DIA).then_some(())?;
    Some(parse_fecha(f)? + h)
}

/// Minutos absolutos → `"2026-10-26T09:30"`.
pub fn formatear(min: i64) -> String {
    let (y, m, d) = civil_desde_dias(min.div_euclid(MIN_DIA));
    let r = min.rem_euclid(MIN_DIA);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}", r / 60, r % 60)
}

/// ¿Se solapan `[a0,a1)` y `[b0,b1)`? (condición clásica de intervalos).
pub fn se_solapan(a0: i64, a1: i64, b0: i64, b1: i64) -> bool {
    a0 < b1 && b0 < a1
}

/// Huecos libres dentro de `[desde, hasta)` dados intervalos ocupados
/// (pueden venir desordenados o solapados entre sí).
pub fn huecos_libres(desde: i64, hasta: i64, ocupados: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let mut occ: Vec<_> = ocupados.iter().copied().filter(|&(a, b)| se_solapan(a, b, desde, hasta)).collect();
    occ.sort_unstable();
    let mut libres = Vec::new();
    let mut cursor = desde;
    for (a, b) in occ {
        if a > cursor {
            libres.push((cursor, a.min(hasta)));
        }
        cursor = cursor.max(b);
    }
    if cursor < hasta {
        libres.push((cursor, hasta));
    }
    libres
}

/// Inicios alineados a `paso` en los que entra una reserva de `duracion`.
pub fn inicios_posibles(libres: &[(i64, i64)], duracion: i64, paso: i64) -> Vec<i64> {
    let mut v = Vec::new();
    for &(a, b) in libres {
        // Redondeamos hacia arriba al múltiplo de `paso`.
        let mut t = a + (paso - a.rem_euclid(paso)) % paso;
        while t + duracion <= b {
            v.push(t);
            t += paso;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fechas_ida_y_vuelta() {
        for s in ["1970-01-01T00:00", "2024-02-29T23:59", "2026-10-26T09:30", "1969-12-31T12:00"] {
            assert_eq!(formatear(parse_fecha_hora(s).unwrap()), s);
        }
        assert_eq!(parse_fecha_hora("1970-01-02T00:00"), Some(MIN_DIA));
        for malo in ["2025-02-29T10:00", "2026-13-01T10:00", "2026-10-26 10:00", "2026-10-26T24:00", "2026-10-26T9:00"] {
            assert_eq!(parse_fecha_hora(malo), None, "{malo}");
        }
        assert_eq!(parse_hora("24:00"), Some(MIN_DIA));
    }

    #[test]
    fn solapamiento_y_huecos() {
        assert!(se_solapan(10, 20, 15, 25));
        assert!(se_solapan(10, 20, 12, 18));
        assert!(!se_solapan(10, 20, 20, 30), "intervalos semiabiertos: tocarse no es chocar");
        let l = huecos_libres(0, 100, &[(50, 60), (10, 20), (15, 30), (90, 120)]);
        assert_eq!(l, vec![(0, 10), (30, 50), (60, 90)]);
        assert_eq!(huecos_libres(0, 10, &[]), vec![(0, 10)]);
        assert_eq!(huecos_libres(0, 10, &[(0, 10)]), vec![]);
        // Con duración 20 y paso 15: en (30,50) entra 30; en (60,90) entran 60 (y 75 no: 95>90).
        assert_eq!(inicios_posibles(&l, 20, 15), vec![30, 60]);
        assert_eq!(inicios_posibles(&[(7, 40)], 10, 15), vec![15, 30]);
    }
}
