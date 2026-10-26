//! Demo de línea de comandos: mina algunos bloques, los imprime, valida la
//! cadena y muestra qué pasa al manipular un bloque.
//!
//! Uso: `cargo run -- [dificultad] [cantidad_bloques]`

use blockchain_minima::Cadena;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn ahora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dificultad: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(3);
    let cantidad: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);

    println!("⛏  Minando con dificultad {dificultad}…");
    let mut cadena = Cadena::nueva(dificultad, ahora());
    for i in 1..=cantidad {
        let t = Instant::now();
        let b = cadena.agregar(format!("transacción #{i}"), ahora());
        println!("{b}  ({:?})", t.elapsed());
    }
    match cadena.validar() {
        Ok(()) => println!("✅ Cadena válida ({} bloques)", cadena.bloques.len()),
        Err(e) => println!("❌ {e}"),
    }
    // Demostración de inmutabilidad: tocamos un bloque viejo.
    if cadena.bloques.len() > 1 {
        cadena.bloques[1].datos.push_str(" (alterado)");
        println!("Tras manipular el bloque 1: {:?}", cadena.validar());
    }
}
