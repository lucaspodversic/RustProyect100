//! Binario `procs`: visor de procesos interactivo. Refresca cada `--intervalo` ms.

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::{prelude::CrosstermBackend, Terminal};
use std::io::{self, stdout};
use std::time::{Duration, Instant};
use visor_procesos::{dibujar, mib, ordenar, App, FuenteProcesos, FuenteSysinfo, Orden};

#[derive(Parser)]
#[command(name = "procs", version, about = "Visor de procesos interactivo (CPU/memoria)")]
struct Args {
    /// Intervalo de refresco en milisegundos.
    #[arg(short, long, default_value_t = 1500)]
    intervalo: u64,
    /// Modo texto: imprime los N procesos con más memoria y sale.
    #[arg(long)]
    top: Option<usize>,
}

fn main() -> io::Result<()> {
    let a = Args::parse();
    if let Some(n) = a.top {
        let mut f = FuenteSysinfo::new();
        let mut v = f.listar();
        ordenar(&mut v, Orden::Memoria);
        println!("{:>8} {:>9}  NOMBRE", "PID", "MEM MiB");
        for p in v.iter().take(n) {
            println!("{:>8} {:>9}  {}", p.pid, mib(p.memoria), p.nombre);
        }
        return Ok(());
    }
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new(FuenteSysinfo::new());
    let tick = Duration::from_millis(a.intervalo);
    let mut ultimo = Instant::now();
    let res = (|| -> io::Result<()> {
        while !app.salir {
            term.draw(|f| dibujar(f, &mut app))?;
            // `poll` con timeout: si no hay teclas, igual refrescamos al vencer el tick.
            let espera = tick.saturating_sub(ultimo.elapsed());
            if event::poll(espera)? {
                if let Event::Key(k) = event::read()? {
                    if k.kind == KeyEventKind::Press {
                        let c = match k.code {
                            KeyCode::Down => 'j',
                            KeyCode::Up => 'k',
                            KeyCode::Enter => '\n',
                            KeyCode::Esc => '\x1b',
                            KeyCode::Backspace => '\x08',
                            KeyCode::Char(c) => c,
                            _ => continue,
                        };
                        app.tecla(c);
                    }
                }
            }
            if ultimo.elapsed() >= tick {
                app.refrescar();
                ultimo = Instant::now();
            }
        }
        Ok(())
    })();
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    res
}
