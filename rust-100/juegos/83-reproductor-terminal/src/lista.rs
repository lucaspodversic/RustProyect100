//! Pistas y lectura de listas M3U / M3U extendido.

#[derive(Debug, Clone, PartialEq)]
pub struct Pista {
    pub ruta: String,
    pub titulo: String,
    pub artista: String,
    /// Duración declarada en la lista (puede faltar: se obtiene al cargar).
    pub duracion_ms: Option<u64>,
}

/// Parsea M3U. Soporta `#EXTINF:segundos,Artista - Título`; las líneas sin
/// metadatos usan el nombre de archivo (sin extensión) como título.
/// Rutas relativas se resuelven contra `base`.
pub fn parsear_m3u(texto: &str, base: &str) -> Vec<Pista> {
    let mut v = Vec::new();
    let mut pendiente: Option<(Option<u64>, String, String)> = None;
    for linea in texto.lines().map(str::trim) {
        if linea.is_empty() {
            continue;
        }
        if let Some(info) = linea.strip_prefix("#EXTINF:") {
            let (seg, resto) = info.split_once(',').unwrap_or((info, ""));
            let dur = seg.trim().parse::<i64>().ok().filter(|s| *s >= 0).map(|s| s as u64 * 1000);
            let (art, tit) = match resto.split_once(" - ") {
                Some((a, t)) => (a.trim().to_string(), t.trim().to_string()),
                None => (String::new(), resto.trim().to_string()),
            };
            pendiente = Some((dur, art, tit));
            continue;
        }
        if linea.starts_with('#') {
            continue; // otros comentarios/directivas
        }
        let ruta = if linea.starts_with('/') || base.is_empty() { linea.to_string() } else { format!("{}/{}", base.trim_end_matches('/'), linea) };
        let nombre = linea.rsplit('/').next().unwrap_or(linea);
        let nombre = nombre.rsplit_once('.').map(|(n, _)| n).unwrap_or(nombre).to_string();
        let (duracion_ms, artista, titulo) = pendiente.take().unwrap_or((None, String::new(), String::new()));
        v.push(Pista { ruta, titulo: if titulo.is_empty() { nombre } else { titulo }, artista, duracion_ms });
    }
    v
}

/// mm:ss (o h:mm:ss si supera la hora).
pub fn formato_tiempo(ms: u64) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}
