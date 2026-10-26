//! Demo headless: reproduce una secuencia de entradas grabada y muestra fotogramas.
use plataformas::fisica::Entrada;
use plataformas::juego::*;
use plataformas::nivel::{Nivel, NIVEL_1};

fn main() {
    let mut j = Juego::new(Nivel::parsear(NIVEL_1).unwrap());
    // "Replay": correr a la derecha saltando de vez en cuando.
    for paso in 0..240 {
        let e = Entrada { der: true, saltar: (paso / 20) % 3 == 0, ..Default::default() };
        for ev in j.actualizar(e) {
            println!("[{:.2}s] {:?}", j.tiempo, ev);
        }
        if paso % 80 == 0 {
            println!("{}\n", j.renderizar(30));
        }
    }
    println!("{}", j.renderizar(30));
}
