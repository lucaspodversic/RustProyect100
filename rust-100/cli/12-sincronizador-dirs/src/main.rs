//! Binario `rsincro`: sincroniza ORIGEN → DESTINO.

use clap::Parser;
use sincronizador_dirs::{ejecutar, planificar, Accion, Opciones};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "rsincro", version, about = "Sincroniza un directorio origen hacia un destino (rsync simplificado)")]
struct Args {
    origen: PathBuf,
    destino: PathBuf,
    /// Borrar en destino lo que no existe en origen.
    #[arg(long)]
    borrar: bool,
    /// Comparar contenido con BLAKE3 en vez de tamaño+fecha.
    #[arg(short = 'c', long)]
    checksum: bool,
    /// Excluir nombres (`.git`) o sufijos (`*.log`). Repetible.
    #[arg(short = 'x', long)]
    excluir: Vec<String>,
    /// Solo mostrar el plan, sin modificar nada.
    #[arg(short = 'n', long)]
    simular: bool,
}

fn main() -> ExitCode {
    let a = Args::parse();
    if !a.origen.is_dir() {
        eprintln!("el origen no es un directorio: {}", a.origen.display());
        return ExitCode::from(2);
    }
    let op = Opciones { borrar: a.borrar, checksum: a.checksum, excluir: a.excluir };
    let plan = match planificar(&a.origen, &a.destino, &op) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error al planificar: {e}");
            return ExitCode::from(1);
        }
    };
    for acc in &plan {
        let (s, p) = match acc {
            Accion::CrearDir(p) => ("+dir", p),
            Accion::Copiar(p) => ("+   ", p),
            Accion::Actualizar(p) => ("~   ", p),
            Accion::Borrar(p) => ("-   ", p),
            Accion::BorrarDir(p) => ("-dir", p),
        };
        println!("{s} {}", p.display());
    }
    if plan.is_empty() {
        println!("Todo sincronizado.");
        return ExitCode::SUCCESS;
    }
    if a.simular {
        println!("(simulación: {} acciones, nada modificado)", plan.len());
        return ExitCode::SUCCESS;
    }
    match ejecutar(&a.origen, &a.destino, &plan) {
        Ok(r) => {
            println!(
                "Listo: {} copiados, {} actualizados, {} borrados, {} dirs, {} bytes",
                r.copiados, r.actualizados, r.borrados, r.dirs_creados, r.bytes
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error al ejecutar: {e}");
            ExitCode::from(1)
        }
    }
}
