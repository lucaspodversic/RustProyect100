//! Firmas HMAC-SHA256 al estilo Stripe/GitHub.
//!
//! Mensaje firmado: `"{timestamp}.{cuerpo}"`. Incluir el timestamp en lo
//! firmado impide que un atacante reutilice una firma vieja con otra fecha
//! (anti-*replay*, junto con la tolerancia de tiempo).

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Devuelve `"sha256=<hex>"`.
pub fn firmar(secreto: &[u8], ts: i64, cuerpo: &[u8]) -> String {
    let mut m = HmacSha256::new_from_slice(secreto).expect("HMAC acepta cualquier largo");
    m.update(ts.to_string().as_bytes());
    m.update(b".");
    m.update(cuerpo);
    format!("sha256={}", hex::encode(m.finalize().into_bytes()))
}

#[derive(Debug, PartialEq)]
pub enum Rechazo {
    FormatoInvalido,
    FirmaIncorrecta,
    FueraDeTiempo,
}

/// Verifica firma y frescura. La comparación es en **tiempo constante**
/// (`verify_slice`) para no filtrar información por *timing*.
pub fn verificar(secreto: &[u8], ts: i64, cuerpo: &[u8], firma: &str, ahora: i64, tolerancia: i64) -> Result<(), Rechazo> {
    if (ahora - ts).abs() > tolerancia {
        return Err(Rechazo::FueraDeTiempo);
    }
    let hexa = firma.strip_prefix("sha256=").ok_or(Rechazo::FormatoInvalido)?;
    let bytes = hex::decode(hexa).map_err(|_| Rechazo::FormatoInvalido)?;
    let mut m = HmacSha256::new_from_slice(secreto).expect("hmac");
    m.update(ts.to_string().as_bytes());
    m.update(b".");
    m.update(cuerpo);
    m.verify_slice(&bytes).map_err(|_| Rechazo::FirmaIncorrecta)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn firma_y_verificacion() {
        let f = firmar(b"s3cr3t", 1000, b"{}");
        assert!(verificar(b"s3cr3t", 1000, b"{}", &f, 1100, 300).is_ok());
        assert_eq!(verificar(b"otro", 1000, b"{}", &f, 1000, 300), Err(Rechazo::FirmaIncorrecta));
        assert_eq!(verificar(b"s3cr3t", 1000, b"{ }", &f, 1000, 300), Err(Rechazo::FirmaIncorrecta));
        // Misma firma con otro timestamp → no vale.
        assert_eq!(verificar(b"s3cr3t", 1001, b"{}", &f, 1000, 300), Err(Rechazo::FirmaIncorrecta));
        assert_eq!(verificar(b"s3cr3t", 1000, b"{}", &f, 2000, 300), Err(Rechazo::FueraDeTiempo));
        assert_eq!(verificar(b"s3cr3t", 1000, b"{}", "md5=xx", 1000, 300), Err(Rechazo::FormatoInvalido));
    }
}
