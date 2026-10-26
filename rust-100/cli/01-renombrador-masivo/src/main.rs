//! Binario `renombrador`: interfaz de línea de comandos sobre la lógica de `lib.rs`.
//!
//! Ejemplos:
//! ```text
//! renombrador ./fotos '^IMG_(\d+)\.jpg$' 'foto-$1.jpg'           # previsualiza (por defecto)
//! renombrador ./fotos '^IMG_(\d+)\.jpg$' 'foto-$1.jpg' --aplicar # ejecuta
//! ```

use clap::Parser;
use regex::Regex;
use renombrador_masivo::{aplicar, formatear_plan, listar_archivos, planificar, Opciones};
use std::path::PathBuf;
use std::process::ExitCode;

/// Argumentos de la CLI, declarados con el derive de clap.
#[derive(Parser, Debug)]
#[command(name = "renombrador", version, about = "Renombrado masivo con regex y previsualización")]
struct Args {
    /// Directorio donde están los archivos.
    dir: PathBuf,
    /// Expresión regular a buscar en cada nombre.
    patron: String,
    /// Reemplazo (admite $1, ${nombre}).
    reemplazo: String,
    /// Aplica los cambios. Sin este flag solo se previsualiza (dry-run seguro por defecto).
    #[arg(long)]
    aplicar: bool,
    /// Reemplaza todas las coincidencias, no solo la primera.
    #[arg(short, long)]
    todas: bool,
    /// Incluye archivos ocultos.
    #[arg(long)]
    ocultos: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    // Compilamos la regex una sola vez; un error de sintaxis se informa claro.
    let re = match Regex::new(&args.patron) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("regex inválida: {e}");
            return ExitCode::from(2);
        }
    };
    let nombres = match listar_archivos(&args.dir) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("no se pudo leer {}: {e}", args.dir.display());
            return ExitCode::from(2);
        }
    };
    let plan = planificar(&nombres, &re, &args.reemplazo, &Opciones { todas: args.todas, incluir_ocultos: args.ocultos });
    if plan.cambios.is_empty() && plan.conflictos.is_empty() {
        println!("Ningún archivo coincide; nada que renombrar.");
        return ExitCode::SUCCESS;
    }
    print!("{}", formatear_plan(&plan));
    if !plan.es_seguro() {
        eprintln!("Hay conflictos: no se aplica ningún cambio.");
        return ExitCode::from(1);
    }
    if !args.aplicar {
        println!("(previsualización: usá --aplicar para ejecutar {} cambios)", plan.cambios.len());
        return ExitCode::SUCCESS;
    }
    match aplicar(&args.dir, &plan) {
        Ok(n) => {
            println!("✔ {n} archivos renombrados.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error al aplicar: {e}");
            ExitCode::from(1)
        }
    }
}
