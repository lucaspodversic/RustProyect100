//! Binario `todo`. El archivo de datos se elige así (en orden):
//! `--archivo`, variable `TODO_ARCHIVO`, o `~/.todo-cli.json`.
//!
//! ```text
//! todo add "Pagar luz" -p alta -v 2024-06-10 -e casa
//! todo ls --pendientes
//! todo done 2
//! ```

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use todo_cli::{Almacen, Filtro, Prioridad};

#[derive(Parser)]
#[command(name = "todo", version, about = "Gestor de tareas con persistencia en JSON")]
struct Cli {
    /// Archivo JSON de datos.
    #[arg(long, global = true, env = "TODO_ARCHIVO")]
    archivo: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Agrega una tarea.
    Add {
        titulo: String,
        #[arg(short, long, default_value = "media")]
        prioridad: Prioridad,
        /// Fecha límite AAAA-MM-DD.
        #[arg(short, long)]
        vence: Option<String>,
        /// Etiqueta (repetible).
        #[arg(short, long)]
        etiqueta: Vec<String>,
    },
    /// Lista tareas con filtros.
    Ls {
        #[arg(long)]
        pendientes: bool,
        #[arg(long)]
        hechas: bool,
        #[arg(short, long)]
        etiqueta: Option<String>,
        #[arg(short, long)]
        prioridad: Option<Prioridad>,
        #[arg(long)]
        vence_hasta: Option<String>,
        #[arg(short, long)]
        buscar: Option<String>,
    },
    /// Marca como hecha.
    Done { id: u32 },
    /// Vuelve a pendiente.
    Undo { id: u32 },
    /// Edita título/prioridad.
    Edit {
        id: u32,
        #[arg(short, long)]
        titulo: Option<String>,
        #[arg(short, long)]
        prioridad: Option<Prioridad>,
    },
    /// Borra una tarea.
    Rm { id: u32 },
    /// Borra todas las completadas.
    Limpiar,
    /// Resumen de totales.
    Stats,
}

/// Ruta por defecto en el HOME del usuario (USERPROFILE en Windows).
fn ruta_por_defecto() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".todo-cli.json")
}

fn correr(cli: Cli) -> Result<(), String> {
    let ruta = cli.archivo.unwrap_or_else(ruta_por_defecto);
    let mut a = Almacen::cargar(&ruta).map_err(|e| e.to_string())?;
    // `cambio` indica si hay que guardar al final.
    let cambio = match cli.cmd {
        Cmd::Add { titulo, prioridad, vence, etiqueta } => {
            let id = a.agregar(&titulo, prioridad, vence, etiqueta)?;
            println!("✔ tarea {id} agregada");
            true
        }
        Cmd::Ls { pendientes, hechas, etiqueta, prioridad, vence_hasta, buscar } => {
            let hechas = match (pendientes, hechas) {
                (true, false) => Some(false),
                (false, true) => Some(true),
                _ => None,
            };
            let f = Filtro { hechas, etiqueta, prioridad_min: prioridad, vence_hasta, texto: buscar };
            let v = a.listar(&f);
            if v.is_empty() {
                println!("(sin tareas)");
            }
            for t in v {
                println!("{t}");
            }
            false
        }
        Cmd::Done { id } => {
            a.marcar(id, true)?;
            println!("✔ {id} completada");
            true
        }
        Cmd::Undo { id } => {
            a.marcar(id, false)?;
            println!("↺ {id} pendiente");
            true
        }
        Cmd::Edit { id, titulo, prioridad } => {
            a.editar(id, titulo, prioridad)?;
            println!("✎ {id} editada");
            true
        }
        Cmd::Rm { id } => {
            let t = a.borrar(id)?;
            println!("🗑 borrada: {}", t.titulo);
            true
        }
        Cmd::Limpiar => {
            println!("🗑 {} completadas borradas", a.limpiar_hechas());
            true
        }
        Cmd::Stats => {
            let (t, h, p) = a.resumen();
            println!("Total {t} · hechas {h} · pendientes {p}");
            false
        }
    };
    if cambio {
        a.guardar(&ruta).map_err(|e| format!("no se pudo guardar {}: {e}", ruta.display()))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match correr(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}
