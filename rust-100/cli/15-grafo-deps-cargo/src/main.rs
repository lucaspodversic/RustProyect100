//! Binario `cargografo`: árbol, DOT, "por qué" y duplicados a partir de un Cargo.lock.

use clap::{Parser, Subcommand};
use grafo_deps_cargo::Grafo;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "cargografo", version, about = "Visualiza el grafo de dependencias de un Cargo.lock")]
struct Args {
    /// Ruta al Cargo.lock (o a la carpeta que lo contiene).
    #[arg(short, long, default_value = ".")]
    lock: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Árbol ASCII desde cada crate del workspace.
    Arbol {
        #[arg(short, long)]
        profundidad: Option<usize>,
    },
    /// Graphviz DOT (pipe a `dot -Tsvg > deps.svg`).
    Dot,
    /// ¿Por qué está esta dependencia? Muestra el camino más corto.
    PorQue { crate_: String },
    /// Crates con varias versiones.
    Duplicados,
    /// Totales y profundidad.
    Stats,
}

fn main() -> ExitCode {
    let a = Args::parse();
    let ruta = if a.lock.is_dir() { a.lock.join("Cargo.lock") } else { a.lock };
    let g = match std::fs::read_to_string(&ruta).map_err(|e| e.to_string()).and_then(|t| Grafo::desde_lock(&t).map_err(|e| e.to_string())) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{}: {e}", ruta.display());
            return ExitCode::from(2);
        }
    };
    match a.cmd {
        Cmd::Arbol { profundidad } => {
            for r in g.raices() {
                println!("{}", g.arbol(r, profundidad));
            }
        }
        Cmd::Dot => print!("{}", g.dot()),
        Cmd::PorQue { crate_ } => {
            let destinos: Vec<usize> = (0..g.nodos.len()).filter(|&i| g.nodos[i].nombre == crate_).collect();
            if destinos.is_empty() {
                eprintln!("'{crate_}' no está en el lock");
                return ExitCode::from(1);
            }
            for d in destinos {
                match g.por_que(d) {
                    Some(c) => println!("{}", c.iter().map(|&i| g.nodos[i].etiqueta()).collect::<Vec<_>>().join(" → ")),
                    None => println!("{}: inalcanzable desde el workspace", g.nodos[d].etiqueta()),
                }
            }
        }
        Cmd::Duplicados => {
            let d = g.duplicados();
            if d.is_empty() {
                println!("Sin versiones duplicadas.");
            }
            for (n, vs) in d {
                println!("{n}: {}", vs.join(", "));
            }
        }
        Cmd::Stats => {
            let e = g.estadisticas();
            println!("paquetes: {}\nraíces (workspace): {}\naristas: {}\nprofundidad máxima: {}", e.paquetes, e.raices, e.aristas, e.profundidad_max);
        }
    }
    ExitCode::SUCCESS
}
