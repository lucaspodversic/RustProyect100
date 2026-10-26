//! Binario `jproc`: mini-jq con subcomandos.
//!
//! ```text
//! jproc get .usuarios[].nombre datos.json
//! jproc filter '.edad >= 30' datos.json --ruta .usuarios
//! jproc pick nombre,edad datos.json --ruta .usuarios
//! jproc set .config.debug true datos.json
//! jproc del .usuarios[0] datos.json
//! cat datos.json | jproc keys
//! ```

use clap::{Parser, Subcommand};
use procesador_json::*;
use serde_json::Value;
use std::io::Read;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "jproc", version, about = "Procesador de JSON: extraer, filtrar y transformar")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
    /// Salida compacta (una línea) en lugar de pretty-print.
    #[arg(short, long, global = true)]
    compacto: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// Extrae valores por ruta (con [] imprime uno por línea).
    Get { ruta: String, archivo: Option<String> },
    /// Filtra un array con una condición `.campo op valor`.
    Filter {
        condicion: String,
        archivo: Option<String>,
        /// Ruta al array a filtrar (por defecto, la raíz).
        #[arg(long, default_value = ".")]
        ruta: String,
    },
    /// Deja solo los campos indicados (separados por coma).
    Pick {
        campos: String,
        archivo: Option<String>,
        #[arg(long, default_value = ".")]
        ruta: String,
    },
    /// Asigna un valor JSON en la ruta (crea objetos intermedios).
    Set { ruta: String, valor: String, archivo: Option<String> },
    /// Borra la clave o índice de la ruta.
    Del { ruta: String, archivo: Option<String> },
    /// Lista las claves de un objeto.
    Keys { archivo: Option<String> },
}

/// Lee el JSON de un archivo o, si no se indica, de stdin.
fn leer(archivo: &Option<String>) -> Result<Value, String> {
    let mut s = String::new();
    match archivo {
        Some(p) if p != "-" => s = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?,
        _ => {
            std::io::stdin().read_to_string(&mut s).map_err(|e| e.to_string())?;
        }
    }
    serde_json::from_str(&s).map_err(|e| format!("JSON inválido: {e}"))
}

fn imprimir(v: &Value, compacto: bool) {
    let s = if compacto { serde_json::to_string(v) } else { serde_json::to_string_pretty(v) };
    println!("{}", s.expect("serializar Value nunca falla"));
}

fn correr(cli: Cli) -> Result<(), String> {
    let e = |x: ErrorJson| x.0;
    match cli.cmd {
        Cmd::Get { ruta, archivo } => {
            let v = leer(&archivo)?;
            for x in extraer(&v, &parsear_ruta(&ruta).map_err(e)?) {
                imprimir(x, cli.compacto);
            }
        }
        Cmd::Filter { condicion, archivo, ruta } => {
            let v = leer(&archivo)?;
            let c = parsear_condicion(&condicion).map_err(e)?;
            let base = extraer(&v, &parsear_ruta(&ruta).map_err(e)?).into_iter().next().ok_or("la ruta no existe")?;
            imprimir(&filtrar(base, &c).map_err(e)?, cli.compacto);
        }
        Cmd::Pick { campos, archivo, ruta } => {
            let v = leer(&archivo)?;
            let cs: Vec<String> = campos.split(',').map(|s| s.trim().to_string()).collect();
            let base = extraer(&v, &parsear_ruta(&ruta).map_err(e)?).into_iter().next().ok_or("la ruta no existe")?;
            imprimir(&elegir(base, &cs), cli.compacto);
        }
        Cmd::Set { ruta, valor, archivo } => {
            let mut v = leer(&archivo)?;
            // Igual que en las condiciones: si no es JSON, es un string.
            let nuevo = serde_json::from_str(&valor).unwrap_or(Value::String(valor));
            asignar(&mut v, &parsear_ruta(&ruta).map_err(e)?, nuevo).map_err(e)?;
            imprimir(&v, cli.compacto);
        }
        Cmd::Del { ruta, archivo } => {
            let mut v = leer(&archivo)?;
            borrar(&mut v, &parsear_ruta(&ruta).map_err(e)?);
            imprimir(&v, cli.compacto);
        }
        Cmd::Keys { archivo } => {
            let v = leer(&archivo)?;
            let o = v.as_object().ok_or("la raíz no es un objeto")?;
            for k in o.keys() {
                println!("{k}");
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match correr(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(m) => {
            eprintln!("error: {m}");
            ExitCode::from(1)
        }
    }
}
