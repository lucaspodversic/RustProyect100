//! Muestra los reportes HID que generaría el macropad al pulsar la macro 1
//! (abrir el lanzador y escribir "terminal").
//! `cargo run --example demo`
use teclado_macros::macros::{Ejecutor, MACROS};

fn main() {
    let mut e = Ejecutor::new(MACROS[1]);
    let mut tick = 0;
    while let Some(r) = e.siguiente() {
        println!("tick {tick:>3}: {:02X?}", r.bytes());
        tick += 1;
    }
}
