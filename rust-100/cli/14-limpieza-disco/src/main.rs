//! Binario `limpiar`: lista (por defecto) o elimina basura regenerable.

use clap::Parser;
use limpieza_disco::*;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

#[derive(Parser)]
#[command(name = "limpiar", version, about = "Limpieza de disco segura (simulación por defecto)")]
struct Args {
    /// Directorios de proyectos a escanear (target/, node_modules/, __pycache__).
    #[arg(short, long)]
    proyectos: Vec<PathBuf>,
    /// Incluir cachés de usuario (~/.cache, cargo, npm...).
    #[arg(long)]
    caches: bool,
    /// Incluir archivos temporales propios en este directorio (ej. /tmp).
    #[arg(long)]
    temporales: Option<PathBuf>,
    /// Antigüedad mínima en días (sin modificaciones).
    #[arg(short, long, default_value_t = 14)]
    dias: u64,
    /// Borrar definitivamente.
    #[arg(long, conflicts_with = "papelera")]
    borrar: bool,
    /// Mover a esta carpeta en lugar de borrar.
    #[arg(long)]
    papelera: Option<PathBuf>,
}

fn main() -> ExitCode {
    let a = Args::parse();
    let ahora = SystemTime::now();
    let edad = Duration::from_secs(a.dias * 86400);
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let mut c = Vec::new();
    // Las raíces permitidas son exactamente las que el usuario pidió escanear.
    let mut raices: Vec<PathBuf> = a.proyectos.clone();
    for p in &a.proyectos {
        c.extend(escanear_proyectos(p, ahora));
    }
    if a.caches {
        if let Some(h) = &home {
            c.extend(caches_usuario(h, ahora));
            raices.push(h.clone());
        }
    }
    if let Some(t) = &a.temporales {
        c.extend(temporales(t, edad, ahora));
        raices.push(t.clone());
    }
    c.retain(|x| x.inactivo >= edad);
    c.sort_by(|x, y| y.bytes.cmp(&x.bytes));
    if c.is_empty() {
        println!("Nada para limpiar (antigüedad mínima: {} días).", a.dias);
        return ExitCode::SUCCESS;
    }
    let total: u64 = c.iter().map(|x| x.bytes).sum();
    for x in &c {
        println!("{:>10}  {:<13} {:>4}d  {}", humano(x.bytes), format!("{:?}", x.categoria), x.inactivo.as_secs() / 86400, x.ruta.display());
    }
    println!("Total recuperable: {}", humano(total));
    let modo = match (a.borrar, a.papelera) {
        (true, _) => Modo::Borrar,
        (_, Some(p)) => Modo::Papelera(p),
        _ => {
            println!("(simulación: usá --borrar o --papelera DIR para aplicar)");
            return ExitCode::SUCCESS;
        }
    };
    let (lib, errores) = limpiar(&c, &raices, home.as_deref(), &modo);
    for (p, e) in &errores {
        eprintln!("omitido {}: {e}", p.display());
    }
    println!("Liberado: {}", humano(lib));
    if errores.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) }
}
