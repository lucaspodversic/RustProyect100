//! Punto de entrada.
//!
//! - `cargo run`             → TUI interactiva (requiere una terminal real).
//! - `cargo run -- --snapshot` → imprime una captura en texto (sin TTY).

use explorador_tui::app::{App, Tecla};
use explorador_tui::datos::CadenaSimulada;
use explorador_tui::{renderizar_texto, ui};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::io::IsTerminal;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let mut app = App::nueva(CadenaSimulada::generar(200, 42));
    if std::env::args().any(|a| a == "--snapshot") || !std::io::stdout().is_terminal() {
        print!("{}", renderizar_texto(&app, 100, 30));
        app.tecla(Tecla::Enter);
        print!("{}", renderizar_texto(&app, 100, 30));
        return Ok(());
    }
    // `ratatui::init` activa modo raw + pantalla alternativa y restaura al salir.
    let mut terminal = ratatui::init();
    let r = (|| -> std::io::Result<()> {
        while !app.salir {
            terminal.draw(|f| ui::dibujar(f, &app))?;
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(k) = event::read()? {
                    if k.kind != KeyEventKind::Press {
                        continue;
                    }
                    let t = match k.code {
                        KeyCode::Up => Tecla::Arriba,
                        KeyCode::Down => Tecla::Abajo,
                        KeyCode::PageUp => Tecla::PagArriba,
                        KeyCode::PageDown => Tecla::PagAbajo,
                        KeyCode::Enter => Tecla::Enter,
                        KeyCode::Esc => Tecla::Esc,
                        KeyCode::Backspace => Tecla::Borrar,
                        KeyCode::Left => Tecla::Car('h'),
                        KeyCode::Right => Tecla::Car('l'),
                        KeyCode::Char(c) => Tecla::Car(c),
                        _ => continue,
                    };
                    app.tecla(t);
                }
            }
        }
        Ok(())
    })();
    ratatui::restore();
    r
}
