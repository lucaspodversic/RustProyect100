//! Demo "fuente": un emisor dispara hacia arriba con gravedad y viento.
//! `cargo run -- [segundos]`
use simulacion_particulas::sistema::*;
use simulacion_particulas::vec2::v2;

fn main() {
    let seg: f32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(3.0);
    let mut s = Sistema::new(60.0, 20.0, 42);
    s.fuerzas.push(Fuerza::Gravedad(v2(0.0, -9.8)));
    s.fuerzas.push(Fuerza::Viento(v2(1.5, 0.0)));
    let mut e = Emisor::new(v2(20.0, 1.0), 120.0, std::f32::consts::FRAC_PI_2, 0.35);
    e.rapidez = (10.0, 15.0);
    s.emisores.push(e);
    s.colisiones = true;
    let dt = 1.0 / 60.0;
    for paso in 0..(seg / dt).round() as usize {
        s.paso(dt);
        if paso % 60 == 59 {
            println!("t = {:.1}s · partículas: {} · energía cinética: {:.0}", (paso + 1) as f32 * dt, s.particulas.len(), s.energia_cinetica());
            print!("{}", s.render(60, 16));
        }
    }
}
