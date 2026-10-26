//! Tabla de récords persistente (JSON). Se escribe de forma **atómica**:
//! primero a un archivo temporal y luego `rename`, para que un corte de luz
//! a mitad de escritura nunca deje el archivo corrupto.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const MAX_ENTRADAS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entrada {
    pub nombre: String,
    pub puntaje: u32,
    /// Segundos desde epoch (sólo informativo).
    pub fecha: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tabla {
    pub entradas: Vec<Entrada>,
}

impl Tabla {
    /// ¿Este puntaje entra en la tabla?
    pub fn califica(&self, puntaje: u32) -> bool {
        puntaje > 0 && (self.entradas.len() < MAX_ENTRADAS || self.entradas.last().is_some_and(|e| puntaje > e.puntaje))
    }

    /// Inserta manteniendo orden descendente (empates: el más antiguo primero).
    /// Devuelve la posición (0 = récord) o `None` si no calificó.
    pub fn agregar(&mut self, nombre: &str, puntaje: u32, fecha: u64) -> Option<usize> {
        if !self.califica(puntaje) {
            return None;
        }
        // Nombre saneado: sin saltos de línea, máx. 16 caracteres.
        let nombre: String = nombre.chars().filter(|c| !c.is_control()).take(16).collect();
        let pos = self.entradas.iter().position(|e| puntaje > e.puntaje).unwrap_or(self.entradas.len());
        self.entradas.insert(pos, Entrada { nombre, puntaje, fecha });
        self.entradas.truncate(MAX_ENTRADAS);
        Some(pos)
    }

    pub fn mejor(&self) -> u32 {
        self.entradas.first().map(|e| e.puntaje).unwrap_or(0)
    }
}

pub struct Almacen {
    pub ruta: PathBuf,
}

impl Almacen {
    pub fn new(ruta: impl AsRef<Path>) -> Self {
        Self { ruta: ruta.as_ref().to_path_buf() }
    }

    /// Ruta por defecto: `$XDG_DATA_HOME/snake/records.json` o `~/.local/share/...`.
    pub fn por_defecto() -> Self {
        let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share"))).unwrap_or_else(|| PathBuf::from("."));
        Self::new(base.join("snake").join("records.json"))
    }

    /// Carga la tabla. Si no existe → vacía. Si está corrupta → se respalda
    /// como `.corrupto` y se empieza de cero (no rompemos el juego por eso).
    pub fn cargar(&self) -> Tabla {
        match fs::read_to_string(&self.ruta) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| {
                let _ = fs::rename(&self.ruta, self.ruta.with_extension("json.corrupto"));
                Tabla::default()
            }),
            Err(_) => Tabla::default(),
        }
    }

    pub fn guardar(&self, t: &Tabla) -> io::Result<()> {
        if let Some(dir) = self.ruta.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.ruta.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(t)?)?;
        fs::rename(tmp, &self.ruta)
    }
}
