//! Demo: `cargo run -- "R U R' U'"` aplica la secuencia y muestra el cubo.
//! Sin argumentos: mezcla de 5 movimientos y la resuelve con IDDFS.
use cubo_rubik::cubo::{a_texto, inversa, orden, parsear, Cubo};
use cubo_rubik::resolver::{mezcla, resolver};
use std::io::IsTerminal;

fn main() {
    let ansi = std::io::stdout().is_terminal();
    let mut c = Cubo::resuelto();
    if let Some(seq) = std::env::args().nth(1) {
        match parsear(&seq) {
            Ok(m) => {
                c.aplicar_seq(&m);
                println!("{}\nOrden de la secuencia: {}\nInversa: {}", c.red(ansi), orden(&m), a_texto(&inversa(&m)));
            }
            Err(e) => eprintln!("Notación inválida: {}", e.0),
        }
        return;
    }
    let m = mezcla(5, 2026);
    c.aplicar_seq(&m);
    println!("Mezcla: {}\n{}", a_texto(&m), c.red(ansi));
    let sol = resolver(&c, 6).expect("debería resolverse");
    c.aplicar_seq(&sol);
    println!("Solución ({} mov.): {}\n{}Resuelto: {}", sol.len(), a_texto(&sol), c.red(ansi), c.esta_resuelto());
}
