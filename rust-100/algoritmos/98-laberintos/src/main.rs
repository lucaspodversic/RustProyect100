//! CLI: genera un laberinto, lo resuelve con los 4 algoritmos y lo dibuja.
//!
//! ```text
//! laberintos [columnas] [filas] [semilla] [--ciclos N] [--barro N] [--archivo ruta.txt]
//! ```
use laberintos::busqueda::{resolver, Algoritmo};
use laberintos::generador::{generar, Opciones};
use laberintos::rejilla::Laberinto;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opcion = |nombre: &str| args.iter().position(|a| a == nombre).and_then(|i| args.get(i + 1));
    let numeros: Vec<u64> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| !a.starts_with("--") && (*i == 0 || !args[i - 1].starts_with("--")))
        .filter_map(|(_, a)| a.parse().ok())
        .collect();
    let lab = if let Some(ruta) = opcion("--archivo") {
        let txt = std::fs::read_to_string(ruta).expect("no se pudo leer el archivo");
        Laberinto::parsear(&txt).expect("formato inválido")
    } else {
        generar(Opciones {
            columnas: numeros.first().copied().unwrap_or(30) as usize,
            filas: numeros.get(1).copied().unwrap_or(12) as usize,
            semilla: numeros.get(2).copied().unwrap_or(2026),
            ciclos: opcion("--ciclos").and_then(|s| s.parse().ok()).unwrap_or(10),
            barro: opcion("--barro").and_then(|s| s.parse().ok()).unwrap_or(10),
        })
    };
    println!("Laberinto {}x{}\n", lab.ancho, lab.alto);
    println!("{:<10} {:>6} {:>6} {:>11} {:>10}", "algoritmo", "pasos", "coste", "expandidos", "tiempo");
    let mut mejor = None;
    for a in [Algoritmo::Bfs, Algoritmo::Dfs, Algoritmo::Dijkstra, Algoritmo::AEstrella] {
        let t = Instant::now();
        match resolver(&lab, a) {
            Some(s) => {
                println!(
                    "{:<10} {:>6} {:>6} {:>11} {:>8.1}µs",
                    format!("{a:?}"),
                    s.camino.len() - 1,
                    s.coste,
                    s.expandidos,
                    t.elapsed().as_secs_f64() * 1e6
                );
                if a == Algoritmo::AEstrella {
                    mejor = Some(s);
                }
            }
            None => println!("{a:?}: sin solución"),
        }
    }
    if let Some(s) = mejor {
        println!("\nCamino de A* (* libre, o barro, ~ barro sin pisar):\n{}", lab.dibujar(&s.camino));
    }
}
