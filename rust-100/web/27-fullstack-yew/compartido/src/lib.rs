//! Tipos y reglas compartidas entre frontend y backend.
//!
//! Ventaja clave del full-stack Rust: la **misma** función de validación
//! corre en el navegador (feedback instantáneo) y en el servidor (la que
//! realmente protege los datos). Imposible que se desincronicen.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Nota {
    pub id: u32,
    pub texto: String,
    pub hecha: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NuevaNota {
    pub texto: String,
}

pub const MAX_TEXTO: usize = 140;

/// Valida y normaliza el texto de una nota.
pub fn validar_texto(t: &str) -> Result<String, String> {
    let t = t.trim();
    if t.is_empty() {
        return Err("la nota no puede estar vacía".into());
    }
    if t.chars().count() > MAX_TEXTO {
        return Err(format!("máximo {MAX_TEXTO} caracteres"));
    }
    Ok(t.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn validacion() {
        assert_eq!(super::validar_texto("  hola "), Ok("hola".into()));
        assert!(super::validar_texto("   ").is_err());
        assert!(super::validar_texto(&"x".repeat(141)).is_err());
    }
}
