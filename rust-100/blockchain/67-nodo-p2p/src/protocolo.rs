//! Mensajes del protocolo de gossip entre nodos.
//!
//! Se transmiten como JSON, **un mensaje por línea** (`\n` como delimitador),
//! lo que hace trivial el framing sobre TCP con `BufRead::lines`.

use crate::cadena::Bloque;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", content = "datos")]
pub enum Mensaje {
    /// Saludo inicial: informa la altura (índice del último bloque) del emisor.
    Hola { altura: u64, direccion: Option<String> },
    /// Pide los bloques a partir de cierto índice (inclusive).
    PedirBloques { desde: u64 },
    /// Respuesta con un tramo de bloques.
    Bloques(Vec<Bloque>),
    /// Anuncio de un bloque recién minado (gossip).
    NuevoBloque(Bloque),
    /// Pide la lista de pares conocidos (descubrimiento).
    PedirPares,
    /// Lista de direcciones de pares.
    Pares(Vec<String>),
}

impl Mensaje {
    /// Codifica el mensaje como una línea JSON terminada en `\n`.
    pub fn a_linea(&self) -> String {
        let mut s = serde_json::to_string(self).expect("los mensajes siempre serializan");
        s.push('\n');
        s
    }

    /// Decodifica una línea JSON.
    pub fn desde_linea(linea: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(linea.trim_end())
    }
}
