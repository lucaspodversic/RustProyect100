//! `cargo run`                → lee /sys real (si no hay GPUs, igual muestra el árbol)
//! `cargo run -- --demo`      → genera un sysfs simulado con 3 GPUs
//! `cargo run -- --sysfs DIR` → usa otra raíz
//! `--texto`                  → sin TUI: imprime árbol y matriz (automático sin TTY)

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::io::IsTerminal;
use std::path::PathBuf;
use topologia_pcie::sysfs::{leer_dispositivos, sysfs_demo};
use topologia_pcie::topologia::Topologia;
use topologia_pcie::ui::{dibujar, App};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let raiz = if args.iter().any(|a| a == "--demo") {
        let d = std::env::temp_dir().join(format!("sysfs-demo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        sysfs_demo(&d)?;
        d
    } else {
        args.iter().position(|a| a == "--sysfs").and_then(|i| args.get(i + 1)).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/sys"))
    };
    let topo = Topologia::new(leer_dispositivos(&raiz)?);
    if args.iter().any(|a| a == "--demo") {
        // El sysfs simulado ya fue leído: lo borramos para no dejar basura en /tmp.
        let _ = std::fs::remove_dir_all(&raiz);
    }
    if args.iter().any(|a| a == "--texto") || !std::io::stdout().is_terminal() {
        for (nivel, d) in topo.arbol() {
            let enlace = d.actual.map(|e| e.to_string()).unwrap_or_default();
            let alerta = d.degradado().map(|w| format!("  ⚠ {w}")).unwrap_or_default();
            println!("{}{} {} [{:06x}] {}{}", "  ".repeat(nivel), d.tipo.icono(), d.bdf, d.clase, enlace, alerta);
        }
        println!("\n{} dispositivos, {} GPUs\n{}", topo.dispositivos.len(), topo.gpus().len(), topo.matriz_texto());
        return Ok(());
    }
    let mut app = App::new(topo);
    let mut term = ratatui::init();
    let res = (|| -> std::io::Result<()> {
        loop {
            term.draw(|f| dibujar(f, &app))?;
            if let Event::Key(k) = event::read()? {
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Down => app.tecla('j'),
                    KeyCode::Up => app.tecla('k'),
                    KeyCode::Char(c) => app.tecla(c),
                    _ => {}
                }
            }
        }
    })();
    ratatui::restore();
    Ok(res?)
}
