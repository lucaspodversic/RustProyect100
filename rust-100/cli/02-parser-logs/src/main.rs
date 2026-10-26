//! Binario `logparse`: filtra un archivo de log (o stdin) y muestra estadísticas.
//!
//! ```text
//! logparse app.log --nivel warn --desde 2024-05-01 --patron 'timeout|refused' --stats
//! cat app.log | logparse - --stats --silencioso
//! ```

use clap::Parser as ClapParser;
use parser_logs::{procesar, Filtro, Nivel};
use regex::Regex;
use std::io::Read;
use std::process::ExitCode;

#[derive(ClapParser, Debug)]
#[command(name = "logparse", version, about = "Filtra logs por nivel/fecha/patrón y muestra estadísticas")]
struct Args {
    /// Archivo de log; usá `-` para leer de stdin.
    archivo: String,
    /// Nivel mínimo: trace, debug, info, warn, error.
    #[arg(short, long)]
    nivel: Option<Nivel>,
    /// Fecha/hora mínima (incluida), formato `AAAA-MM-DD[ HH:MM:SS]`.
    #[arg(long)]
    desde: Option<String>,
    /// Fecha/hora máxima (incluida).
    #[arg(long)]
    hasta: Option<String>,
    /// Regex sobre el mensaje.
    #[arg(short, long)]
    patron: Option<String>,
    /// Muestra estadísticas al final.
    #[arg(short, long)]
    stats: bool,
    /// No imprime las líneas coincidentes (útil con --stats).
    #[arg(short = 'q', long)]
    silencioso: bool,
}

fn main() -> ExitCode {
    let a = Args::parse();
    // Leemos todo el contenido: simple y suficiente para logs de tamaño moderado.
    let mut texto = String::new();
    let res = if a.archivo == "-" {
        std::io::stdin().read_to_string(&mut texto).map(|_| ())
    } else {
        std::fs::read_to_string(&a.archivo).map(|t| texto = t)
    };
    if let Err(e) = res {
        eprintln!("no se pudo leer {}: {e}", a.archivo);
        return ExitCode::from(2);
    }
    let patron = match a.patron.as_deref().map(Regex::new).transpose() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("patrón inválido: {e}");
            return ExitCode::from(2);
        }
    };
    let filtro = Filtro { nivel_min: a.nivel, desde: a.desde, hasta: a.hasta, patron };
    let (entradas, st) = procesar(texto.lines(), &filtro);
    if !a.silencioso {
        for e in &entradas {
            println!("{} {:<5} {}", e.marca(), e.nivel, e.mensaje);
        }
    }
    if a.stats {
        println!("\n{}", st.informe());
    }
    ExitCode::SUCCESS
}
