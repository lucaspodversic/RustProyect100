//! Binario `dutui`: escanea un directorio y abre la TUI.
//!
//! Con `--texto` imprime el top-N sin abrir la interfaz (útil en scripts o sin TTY).

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, stdout};
use std::path::PathBuf;
use visor_disco_tui::{dibujar, escanear, humano, App};

#[derive(Parser, Debug)]
#[command(name = "dutui", version, about = "Visor de uso de disco con TUI")]
struct Args {
    /// Directorio a analizar.
    #[arg(default_value = ".")]
    ruta: PathBuf,
    /// Modo texto: imprime los N más pesados y sale.
    #[arg(long)]
    texto: Option<usize>,
}

fn main() -> io::Result<()> {
    let a = Args::parse();
    eprintln!("Escaneando {} ...", a.ruta.display());
    let raiz = escanear(&a.ruta);
    if let Some(n) = a.texto {
        for h in raiz.hijos.iter().take(n) {
            println!("{:>10}  {}", humano(h.tamano), h.nombre);
        }
        return Ok(());
    }
    // Preparamos la terminal: modo raw + pantalla alternativa (se restaura al salir).
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new(raiz);
    let res = (|| -> io::Result<()> {
        while !app.salir {
            term.draw(|f| dibujar(f, &mut app))?;
            if let Event::Key(k) = event::read()? {
                // En Windows llegan Press y Release: procesamos solo Press.
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                // Traducimos teclas de crossterm al alfabeto simple de `App::tecla`.
                let c = match k.code {
                    KeyCode::Down => 'j',
                    KeyCode::Up => 'k',
                    KeyCode::Enter | KeyCode::Right => 'l',
                    KeyCode::Backspace | KeyCode::Left => 'h',
                    KeyCode::Esc => 'q',
                    KeyCode::Char(c) => c,
                    _ => continue,
                };
                app.tecla(c);
            }
        }
        Ok(())
    })();
    // Restauramos SIEMPRE la terminal, aunque haya habido error.
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    res
}
