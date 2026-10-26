//! Demo headless: el piloto automático juega N segundos y se imprimen
//! algunos fotogramas. `cargo run -- [segundos] [--ansi]`.
use shooter_2d::juego::*;
use shooter_2d::motor::Framebuffer;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let segundos: f32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20.0);
    let ansi = args.iter().any(|a| a == "--ansi");
    let mut j = Juego::new(42);
    let mut fb = Framebuffer::new(ANCHO as usize, ALTO as usize);
    let pasos = (segundos / DT) as usize;
    for paso in 0..pasos {
        let e = piloto_automatico(&j);
        for ev in j.actualizar(e) {
            if !matches!(ev, EventoJuego::EnemigoDestruido { .. }) {
                println!("[t={:.1}s] {:?}", j.tiempo, ev);
            }
        }
        if paso % (pasos / 3).max(1) == 0 || j.terminado {
            j.renderizar(&mut fb);
            println!("{}", if ansi { fb.a_ansi() } else { fb.a_texto() });
        }
        if j.terminado {
            break;
        }
    }
    println!("Puntaje final: {} (oleada {}, vidas {})", j.puntaje, j.oleada, j.vidas);
}
