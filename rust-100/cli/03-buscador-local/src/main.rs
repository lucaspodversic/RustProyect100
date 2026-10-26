//! Binario `buscar`: busca por nombre y/o contenido en un árbol de directorios.
//!
//! ```text
//! buscar TODO                 # contenido en el directorio actual
//! buscar TODO src -e rs       # solo .rs bajo src/
//! buscar -n '^main' .         # solo por nombre (lista archivos)
//! ```

use buscador_local::{buscar_contenido, recolectar, resaltar, Opciones};
use clap::Parser;
use regex::RegexBuilder;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "buscar", version, about = "Buscador local tipo ripgrep (nombre + contenido)")]
struct Args {
    /// Regex a buscar en el contenido. Si se omite, solo se listan archivos.
    patron: Option<String>,
    /// Directorio raíz (por defecto, el actual).
    #[arg(default_value = ".")]
    raiz: PathBuf,
    /// Regex sobre el nombre de archivo.
    #[arg(short, long)]
    nombre: Option<String>,
    /// Extensión permitida (repetible): -e rs -e toml
    #[arg(short, long)]
    ext: Vec<String>,
    /// Ignorar mayúsculas/minúsculas en el patrón de contenido.
    #[arg(short, long)]
    ignorar_caso: bool,
    /// Incluir ocultos.
    #[arg(long)]
    ocultos: bool,
    /// Profundidad máxima.
    #[arg(short = 'd', long)]
    profundidad: Option<usize>,
    /// Solo imprime nombres de archivo con coincidencias.
    #[arg(short = 'l', long)]
    solo_archivos: bool,
}

fn main() -> ExitCode {
    let a = Args::parse();
    let nombre = match a.nombre.as_deref().map(regex::Regex::new).transpose() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("regex de nombre inválida: {e}");
            return ExitCode::from(2);
        }
    };
    let o = Opciones { nombre, extensiones: a.ext, ocultos: a.ocultos, max_profundidad: a.profundidad, max_bytes: 50 * 1024 * 1024 };
    let archivos = recolectar(&a.raiz, &o);
    let Some(p) = a.patron else {
        // Modo "find": solo listamos.
        for f in &archivos {
            println!("{}", f.display());
        }
        return if archivos.is_empty() { ExitCode::from(1) } else { ExitCode::SUCCESS };
    };
    let re = match RegexBuilder::new(&p).case_insensitive(a.ignorar_caso).build() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("regex inválida: {e}");
            return ExitCode::from(2);
        }
    };
    let m = buscar_contenido(&archivos, &re, o.max_bytes);
    // Colores solo si stdout es una terminal (no al redirigir a archivo).
    let color = std::io::stdout().is_terminal();
    let mut ultimo: Option<&PathBuf> = None;
    for c in &m {
        if a.solo_archivos {
            if ultimo != Some(&c.ruta) {
                println!("{}", c.ruta.display());
            }
        } else {
            let t = if color { resaltar(&c.texto, &re) } else { c.texto.clone() };
            println!("{}:{}:{}", c.ruta.display(), c.linea, t);
        }
        ultimo = Some(&c.ruta);
    }
    // Código de salida estilo grep: 0 si hubo coincidencias, 1 si no.
    if m.is_empty() { ExitCode::from(1) } else { ExitCode::SUCCESS }
}
