//! Detección de tipo por **contenido** ("magic bytes").
//!
//! Nunca confiamos en el `Content-Type` ni en la extensión que manda el
//! cliente: un `.png` puede ser un ejecutable. Miramos los primeros bytes.

/// Devuelve el MIME detectado, o `None` si no se reconoce.
pub fn detectar(datos: &[u8]) -> Option<&'static str> {
    const FIRMAS: &[(&[u8], &str)] = &[
        (b"\x89PNG\r\n\x1a\n", "image/png"),
        (b"\xFF\xD8\xFF", "image/jpeg"),
        (b"GIF87a", "image/gif"),
        (b"GIF89a", "image/gif"),
        (b"%PDF-", "application/pdf"),
        (b"PK\x03\x04", "application/zip"),
        (b"\x7FELF", "application/x-elf"),
        (b"MZ", "application/x-msdownload"),
    ];
    for (firma, mime) in FIRMAS {
        if datos.starts_with(firma) {
            return Some(mime);
        }
    }
    // WebP: "RIFF....WEBP"
    if datos.len() >= 12 && &datos[..4] == b"RIFF" && &datos[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    // Texto: UTF-8 válido y sin bytes de control raros (salvo \t \n \r).
    if !datos.is_empty()
        && std::str::from_utf8(datos).is_ok()
        && !datos.iter().any(|&b| b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r'))
    {
        return Some("text/plain");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn firmas() {
        assert_eq!(detectar(b"\x89PNG\r\n\x1a\n...."), Some("image/png"));
        assert_eq!(detectar(b"%PDF-1.7"), Some("application/pdf"));
        assert_eq!(detectar(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(detectar("hola ñandú\n".as_bytes()), Some("text/plain"));
        assert_eq!(detectar(b"MZ\x90\0"), Some("application/x-msdownload"));
        assert_eq!(detectar(b"\0\x01\x02"), None);
    }
}
