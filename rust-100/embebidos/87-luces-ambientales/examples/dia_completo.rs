//! Recorre un día completo cada 30 minutos y muestra el color resultante
//! como bloque de color ANSI en la terminal. `cargo run --example dia_completo`
use luces_ambientales::horario::{color_para, CURVA_CIRCADIANA};

fn main() {
    for media_hora in 0..48u16 {
        let m = media_hora * 30;
        let c = color_para(&CURVA_CIRCADIANA, m);
        println!(
            "{:02}:{:02}  \x1b[48;2;{};{};{}m        \x1b[0m  rgb({:3},{:3},{:3})",
            m / 60, m % 60, c.r, c.g, c.b, c.r, c.g, c.b
        );
    }
}
