//! En una terminal: interfaz interactiva con backend simulado (el tiempo
//! avanza en tiempo real). Sin TTY o con `--demo`: guion automático que
//! imprime cuadros de la interfaz en texto.
//!
//! `cargo run -- [lista.m3u] [--demo]`

use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Terminal;
use reproductor_terminal::audio::BackendSimulado;
use reproductor_terminal::lista::{parsear_m3u, Pista};
use reproductor_terminal::reproductor::{Comando, Reproductor};
use reproductor_terminal::ui::dibujar;
use std::io::IsTerminal;
use std::time::{Duration, Instant};

const DEMO_M3U: &str = "#EXTM3U
#EXTINF:185,Trío Sintético - Amanecer en Bytes
amanecer.ogg
#EXTINF:242,Los Compiladores - Préstamo Mutable
prestamo.ogg
#EXTINF:96,Orquesta Async - Futuro Pendiente
futuro.ogg
#EXTINF:301,Banda Lifetime - 'static
static.ogg
";

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let demo = args.iter().any(|a| a == "--demo") || !std::io::stdout().is_terminal();
    let pistas: Vec<Pista> = match args.iter().find(|a| !a.starts_with("--")) {
        Some(ruta) => {
            let base = std::path::Path::new(ruta).parent().map(|p| p.display().to_string()).unwrap_or_default();
            parsear_m3u(&std::fs::read_to_string(ruta)?, &base)
        }
        None => parsear_m3u(DEMO_M3U, ""),
    };
    // El backend simulado "conoce" cada pista con la duración declarada (o 3 min).
    let catalogo: Vec<(String, u64)> = pistas.iter().map(|p| (p.ruta.clone(), p.duracion_ms.unwrap_or(180_000))).collect();
    let refs: Vec<(&str, u64)> = catalogo.iter().map(|(r, d)| (r.as_str(), *d)).collect();
    let mut rep = Reproductor::new(BackendSimulado::con_catalogo(&refs), pistas);

    if demo {
        let mut term = Terminal::new(TestBackend::new(70, 12)).unwrap();
        let guion = [(Comando::PlayPausa, 30_000), (Comando::Adelantar(5_000), 2_000), (Comando::Siguiente, 61_000), (Comando::Aleatorio, 1_000), (Comando::Silencio, 0)];
        for (c, ms) in guion {
            rep.ejecutar(c);
            rep.tick(ms);
            term.draw(|f| dibujar(f, &rep)).unwrap();
            println!("== tras {c:?} ==\n{}", texto(term.backend()));
        }
        return Ok(());
    }

    let mut term = ratatui::init();
    let mut ultimo = Instant::now();
    let res = (|| -> std::io::Result<()> {
        loop {
            term.draw(|f| dibujar(f, &rep))?;
            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(k) = event::read()? {
                    if k.kind == KeyEventKind::Press {
                        match k.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Char(c) => {
                                if let Some(cmd) = Comando::desde_tecla(c) {
                                    rep.ejecutar(cmd)
                                }
                            }
                            KeyCode::Right => rep.ejecutar(Comando::Adelantar(5_000)),
                            KeyCode::Left => rep.ejecutar(Comando::Retroceder(5_000)),
                            _ => {}
                        }
                    }
                }
            }
            let ahora = Instant::now();
            rep.tick((ahora - ultimo).as_millis() as u64);
            ultimo = ahora;
        }
    })();
    ratatui::restore();
    res
}

fn texto(b: &TestBackend) -> String {
    let buf = b.buffer();
    (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>().trim_end().to_string() + "\n").collect()
}
