//! Binario `logwatch`:
//! ```text
//! logwatch /var/log/app.log 'ERROR|panic' -- 'notify-send "Falla" "$WATCH_LINE"'
//! ```

use clap::Parser;
use regex::Regex;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};
use watcher_logs::{paso, Disparador, EjecutorShell, Seguidor};

#[derive(Parser)]
#[command(name = "logwatch", version, about = "Vigila un log y ejecuta un comando cuando aparece un patrón")]
struct Args {
    /// Archivo a vigilar.
    archivo: PathBuf,
    /// Regex que dispara la acción.
    patron: String,
    /// Comando a ejecutar (la línea está en $WATCH_LINE).
    comando: String,
    /// Segundos mínimos entre disparos.
    #[arg(long, default_value_t = 0)]
    cooldown: u64,
    /// Termina tras N disparos.
    #[arg(long)]
    max: Option<u64>,
    /// Procesa también el contenido existente (por defecto solo lo nuevo).
    #[arg(long)]
    desde_inicio: bool,
    /// Intervalo de sondeo en milisegundos.
    #[arg(long, default_value_t = 500)]
    intervalo: u64,
}

fn main() -> ExitCode {
    let a = Args::parse();
    let re = match Regex::new(&a.patron) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("patrón inválido: {e}");
            return ExitCode::from(2);
        }
    };
    let mut seg = match Seguidor::new(&a.archivo, a.desde_inicio) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("no se puede abrir {}: {e}", a.archivo.display());
            return ExitCode::from(2);
        }
    };
    let mut disp = Disparador::new(re, Duration::from_secs(a.cooldown), a.max);
    eprintln!("Vigilando {} (Ctrl+C para salir)...", a.archivo.display());
    loop {
        match paso(&mut seg, &mut disp, &mut EjecutorShell, &a.comando, Instant::now()) {
            Ok(n) if n > 0 => eprintln!("→ disparado {n} vez/veces (total {})", disp.disparos),
            Ok(_) => {}
            Err(e) => eprintln!("error: {e}"),
        }
        if disp.agotado() {
            eprintln!("Máximo de disparos alcanzado.");
            return ExitCode::SUCCESS;
        }
        std::thread::sleep(Duration::from_millis(a.intervalo));
    }
}
