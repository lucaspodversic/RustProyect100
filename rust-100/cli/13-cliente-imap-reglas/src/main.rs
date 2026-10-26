//! Binario `imapreglas`: conecta a un servidor IMAP (o al simulado con `--demo`)
//! y aplica reglas JSON a la bandeja.

use clap::Parser;
use cliente_imap_reglas::{imap::Cliente, procesar, reglas, servidor_simulado};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "imapreglas", version, about = "Aplica reglas de filtrado a un buzón IMAP")]
struct Args {
    /// Archivo JSON de reglas.
    #[arg(short, long)]
    reglas: PathBuf,
    /// host:puerto del servidor IMAP (texto plano, ej. vía túnel stunnel al 993).
    #[arg(long, default_value = "127.0.0.1:143")]
    servidor: String,
    #[arg(long, env = "IMAP_USUARIO", default_value = "")]
    usuario: String,
    /// Mejor por variable de entorno que por argumento (no queda en el historial).
    #[arg(long, env = "IMAP_CLAVE", default_value = "", hide_env_values = true)]
    clave: String,
    #[arg(long, default_value = "INBOX")]
    carpeta: String,
    /// Criterio IMAP: ALL o UNSEEN.
    #[arg(long, default_value = "ALL")]
    criterio: String,
    /// Mostrar qué se haría sin modificar nada.
    #[arg(short = 'n', long)]
    simular: bool,
    /// Usar un servidor IMAP simulado local con correos de ejemplo.
    #[arg(long)]
    demo: bool,
}

fn main() -> ExitCode {
    let a = Args::parse();
    let rs = match std::fs::read_to_string(&a.reglas).map_err(|e| e.to_string()).and_then(|t| reglas::cargar(&t).map_err(|e| e.to_string())) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("reglas inválidas: {e}");
            return ExitCode::from(2);
        }
    };
    let (servidor, usuario, clave) = if a.demo {
        let dir = servidor_simulado::iniciar(servidor_simulado::buzon_demo(), "demo", "demo");
        (dir.to_string(), "demo".to_string(), "demo".to_string())
    } else {
        (a.servidor, a.usuario, a.clave)
    };
    let r = (|| -> Result<_, Box<dyn std::error::Error>> {
        let mut c = Cliente::nuevo(TcpStream::connect(&servidor)?)?;
        c.login(&usuario, &clave)?;
        let n = c.seleccionar(&a.carpeta)?;
        println!("{} mensajes en {}", n, a.carpeta);
        let hechas = procesar(&mut c, &rs, &a.criterio, a.simular)?;
        c.logout()?;
        Ok(hechas)
    })();
    match r {
        Ok(h) => {
            for x in &h {
                println!("uid {:>4}  [{}] {:?}  «{}»", x.uid, x.regla, x.accion, x.asunto);
            }
            println!("{} mensajes {}.", h.len(), if a.simular { "coincidirían (simulación)" } else { "procesados" });
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}
