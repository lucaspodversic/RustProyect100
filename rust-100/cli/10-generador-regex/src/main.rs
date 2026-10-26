//! Binario `regexgen`:
//! ```text
//! regexgen 2024-01-05 1999-12-31 -d -r      → ^\d{4}\-\d{2}\-\d{2}$
//! regexgen -f ejemplos.txt                  (un ejemplo por línea)
//! ```

use clap::Parser;
use generador_regex::{generar, Opciones};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "regexgen", version, about = "Genera una regex a partir de ejemplos (tipo grex)")]
struct Args {
    /// Ejemplos a aceptar.
    ejemplos: Vec<String>,
    /// Archivo con un ejemplo por línea.
    #[arg(short, long)]
    archivo: Option<String>,
    /// Generaliza dígitos a \d.
    #[arg(short, long)]
    digitos: bool,
    /// Generaliza letras/dígitos a \w.
    #[arg(short, long)]
    palabras: bool,
    /// Generaliza espacios a \s.
    #[arg(short, long)]
    espacios: bool,
    /// Comprime repeticiones con {n}.
    #[arg(short, long)]
    repeticiones: bool,
    /// Insensible a mayúsculas.
    #[arg(short, long)]
    ignorar_caso: bool,
}

fn main() -> ExitCode {
    let a = Args::parse();
    let mut ej = a.ejemplos.clone();
    if let Some(p) = &a.archivo {
        match std::fs::read_to_string(p) {
            Ok(t) => ej.extend(t.lines().filter(|l| !l.is_empty()).map(String::from)),
            Err(e) => {
                eprintln!("{p}: {e}");
                return ExitCode::from(2);
            }
        }
    }
    let o = Opciones { digitos: a.digitos, palabras: a.palabras, espacios: a.espacios, repeticiones: a.repeticiones, ignorar_caso: a.ignorar_caso };
    match generar(&ej, &o) {
        Ok(r) => {
            println!("{r}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}
