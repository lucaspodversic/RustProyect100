//! Snake en la terminal.
//! - `cargo run` en una TTY: jugás con flechas/WASD, `q` sale.
//! - `cargo run -- --auto` (o sin TTY): juega la IA y se guarda el récord.
//! - `--records RUTA` cambia el archivo de récords.

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::{cursor, execute, terminal};
use snake_highscore::ia::decidir;
use snake_highscore::juego::*;
use snake_highscore::records::Almacen;
use std::io::{stdout, IsTerminal, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let auto = args.iter().any(|a| a == "--auto") || !stdout().is_terminal();
    let almacen = args.iter().position(|a| a == "--records").and_then(|i| args.get(i + 1)).map(Almacen::new).unwrap_or_else(Almacen::por_defecto);
    let semilla = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    let mut s = Snake::new(20, 12, Modo::Paredes, semilla);

    if auto {
        while s.vivo && s.ticks < 5_000 {
            let d = decidir(&s);
            s.girar(d);
            if s.paso() == Resultado::Gano {
                break;
            }
        }
        println!("{}", s.renderizar());
    } else {
        jugar(&mut s)?;
    }

    let mut tabla = almacen.cargar();
    let ahora = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let nombre = std::env::var("USER").unwrap_or_else(|_| "jugador".into());
    match tabla.agregar(if auto { "IA" } else { &nombre }, s.puntaje, ahora) {
        Some(0) => println!("¡NUEVO RÉCORD! {}", s.puntaje),
        Some(p) => println!("Entraste al top en el puesto {}", p + 1),
        None => println!("Puntaje {} (récord: {})", s.puntaje, tabla.mejor()),
    }
    almacen.guardar(&tabla)?;
    println!("Récords en {}:", almacen.ruta.display());
    for (i, e) in tabla.entradas.iter().enumerate() {
        println!("{:>2}. {:<16} {}", i + 1, e.nombre, e.puntaje);
    }
    Ok(())
}

fn jugar(s: &mut Snake) -> std::io::Result<()> {
    let mut out = stdout();
    terminal::enable_raw_mode()?;
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide)?;
    let mut tick = 0u32;
    let res = (|| -> std::io::Result<()> {
        while s.vivo {
            if event::poll(Duration::from_millis(25))? {
                if let Event::Key(k) = event::read()? {
                    if k.kind == KeyEventKind::Press {
                        match k.code {
                            KeyCode::Up | KeyCode::Char('w') => s.girar(Dir::Arriba),
                            KeyCode::Down | KeyCode::Char('s') => s.girar(Dir::Abajo),
                            KeyCode::Left | KeyCode::Char('a') => s.girar(Dir::Izq),
                            KeyCode::Right | KeyCode::Char('d') => s.girar(Dir::Der),
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            _ => {}
                        }
                    }
                }
            }
            tick += 1;
            if tick >= s.intervalo() {
                tick = 0;
                if s.paso() == Resultado::Gano {
                    break;
                }
            }
            execute!(out, cursor::MoveTo(0, 0))?;
            write!(out, "{}", s.renderizar().replace('\n', "\r\n"))?;
            out.flush()?;
        }
        Ok(())
    })();
    execute!(out, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    res
}
