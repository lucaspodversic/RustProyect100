//! Funciones puras de analítica (sin base de datos → fáciles de testear).

/// Clase de dispositivo a partir del User-Agent (heurística simple).
pub fn dispositivo(ua: &str) -> &'static str {
    let u = ua.to_ascii_lowercase();
    if u.is_empty() {
        "desconocido"
    } else if ["bot", "crawler", "spider", "curl", "wget", "python-requests"].iter().any(|b| u.contains(b)) {
        "bot"
    } else if ["mobile", "android", "iphone"].iter().any(|m| u.contains(m)) {
        "movil"
    } else if u.contains("ipad") || u.contains("tablet") {
        "tablet"
    } else {
        "escritorio"
    }
}

/// Reduce un Referer a su host (sin path ni query: puede contener datos
/// personales). Sin referer → "directo".
pub fn host_referrer(r: Option<&str>) -> String {
    r.and_then(|r| url::Url::parse(r).ok())
        .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_ascii_lowercase()))
        .unwrap_or_else(|| "directo".into())
}

/// Identificador anónimo de visitante: hash(sal, ip, ua, día).
/// Incluir el día hace que no se pueda seguir a alguien entre días, y la
/// sal secreta impide revertir el hash probando IPs.
pub fn visitante(sal: &str, ip: &str, ua: &str, dia: &str) -> String {
    let h = blake3::hash(format!("{sal}|{ip}|{ua}|{dia}").as_bytes());
    h.to_hex()[..16].to_string()
}

/// Días desde 1970-01-01 → "AAAA-MM-DD" (algoritmo civil de H. Hinnant).
pub fn dia_de(ts: i64) -> String {
    let z = ts.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utilidades() {
        assert_eq!(dispositivo("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0)"), "movil");
        assert_eq!(dispositivo("Googlebot/2.1"), "bot");
        assert_eq!(dispositivo("Mozilla/5.0 (X11; Linux x86_64)"), "escritorio");
        assert_eq!(host_referrer(Some("https://www.Google.com/search?q=secreto")), "google.com");
        assert_eq!(host_referrer(Some("basura")), "directo");
        assert_eq!(host_referrer(None), "directo");
        assert_eq!(dia_de(0), "1970-01-01");
        assert_eq!(dia_de(1_709_164_800), "2024-02-29");
        assert_ne!(visitante("s", "1.1.1.1", "ua", "2026-01-01"), visitante("s", "1.1.1.1", "ua", "2026-01-02"));
    }
}
