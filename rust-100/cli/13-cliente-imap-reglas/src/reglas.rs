//! Motor de reglas: puro, sin red. Dado un mensaje, decide qué acción aplicar.
//!
//! Formato JSON de ejemplo (`reglas.json`):
//! ```json
//! [
//!   {"nombre": "facturas", "de_contiene": "facturacion@", "accion": {"mover": "Facturas"}},
//!   {"nombre": "promos", "asunto_contiene": "oferta", "accion": "marcar_leido"},
//!   {"nombre": "spam", "de_contiene": "@spam.example", "asunto_contiene": "ganaste", "accion": "borrar"}
//! ]
//! ```
//! Todas las condiciones presentes deben cumplirse (AND), sin distinguir mayúsculas.
//! Gana la **primera** regla que coincide (como los filtros de Gmail/Thunderbird con "detener").

use serde::{Deserialize, Serialize};

/// Cabeceras relevantes de un mensaje.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mensaje {
    pub uid: u32,
    pub de: String,
    pub asunto: String,
}

/// Qué hacer con un mensaje que coincide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accion {
    /// Mover a otra carpeta (COPY + \Deleted + EXPUNGE).
    Mover(String),
    /// Agregar la bandera \Seen.
    MarcarLeido,
    /// Agregar la bandera \Flagged (estrella).
    Destacar,
    /// Marcar \Deleted (se elimina en el EXPUNGE final).
    Borrar,
}

/// Una regla de filtrado.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regla {
    pub nombre: String,
    #[serde(default)]
    pub de_contiene: Option<String>,
    #[serde(default)]
    pub asunto_contiene: Option<String>,
    pub accion: Accion,
}

fn contiene(h: &str, aguja: &Option<String>) -> bool {
    aguja.as_ref().is_none_or(|a| h.to_lowercase().contains(&a.to_lowercase()))
}

impl Regla {
    /// ¿El mensaje cumple todas las condiciones? Una regla sin condiciones no aplica
    /// (evita que un error de configuración mueva TODO el buzón).
    pub fn coincide(&self, m: &Mensaje) -> bool {
        if self.de_contiene.is_none() && self.asunto_contiene.is_none() {
            return false;
        }
        contiene(&m.de, &self.de_contiene) && contiene(&m.asunto, &self.asunto_contiene)
    }
}

/// Primera regla que coincide.
pub fn evaluar<'a>(reglas: &'a [Regla], m: &Mensaje) -> Option<&'a Regla> {
    reglas.iter().find(|r| r.coincide(m))
}

/// Parsea las reglas desde JSON.
pub fn cargar(json: &str) -> Result<Vec<Regla>, serde_json::Error> {
    serde_json::from_str(json)
}
