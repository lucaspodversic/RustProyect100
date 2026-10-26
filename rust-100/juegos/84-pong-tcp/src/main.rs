//! Modos:
//!   cargo run -- servidor [127.0.0.1:7878] [puntos]
//!   cargo run -- cliente  [127.0.0.1:7878] [nombre]   (IA; dibuja la cancha)
//!   cargo run -- local                                 (servidor + 2 IA en este proceso)
//!
//! El cliente juega con IA para poder probarse sin teclado; para jugar a
//! mano bastaría con otro `Controlador` que lea teclas (p. ej. con crossterm).

use pong_tcp::juego::render;
use pong_tcp::red::{controlador_ia, jugar, servir, Config};
use std::net::TcpListener;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let modo = args.first().map(String::as_str).unwrap_or("local");
    let dir = args.get(1).cloned().unwrap_or_else(|| "127.0.0.1:7878".into());
    match modo {
        "servidor" => {
            let l = TcpListener::bind(&dir)?;
            println!("Esperando 2 jugadores en {dir}…");
            let pts = args.get(2).and_then(|p| p.parse().ok()).unwrap_or(5);
            let r = servir(l, Config { para_ganar: pts, ..Config::default() })?;
            println!("{r:?}");
        }
        "cliente" => {
            let nombre = args.get(2).cloned().unwrap_or_else(|| "jugador".into());
            let mut n = 0u64;
            let r = jugar(&dir, &nombre, controlador_ia(0.8), |e| {
                n += 1;
                if n % 6 == 0 {
                    print!("\x1b[H\x1b[2J{}", render(e, 60, 18));
                }
            })?;
            println!("Fin: {:?}", r.fin);
        }
        _ => {
            // Partida completa en localhost con puerto efímero.
            let l = TcpListener::bind("127.0.0.1:0")?;
            let addr = l.local_addr()?.to_string();
            println!("Servidor local en {addr}");
            let cfg = Config { para_ganar: 3, tick_real: Duration::ZERO, max_ticks: 60_000, ..Config::default() };
            let srv = std::thread::spawn(move || servir(l, cfg));
            let a2 = addr.clone();
            let c1 = std::thread::spawn(move || jugar(&a2, "Izquierda", controlador_ia(0.5), |_| {}));
            std::thread::sleep(Duration::from_millis(50));
            let mut ultimo = None;
            let r2 = jugar(&addr, "Derecha", controlador_ia(2.0), |e| ultimo = Some(e.clone()))?;
            let r1 = c1.join().unwrap()?;
            let res = srv.join().unwrap()?;
            if let Some(e) = ultimo {
                print!("{}", render(&e, 60, 16));
            }
            println!("Resultado: {:?} ({} ticks). Cliente 1 vio {} estados, cliente 2 vio {}.", res.puntos, res.ticks, r1.estados, r2.estados);
            match res.ganador {
                Some(g) => println!("Ganó {}", res.nombres[g]),
                None => println!("Empate"),
            }
        }
    }
    Ok(())
}
