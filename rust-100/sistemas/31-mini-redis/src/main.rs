//! Binario `mini-redis`:
//!   mini-redis server [addr] [snapshot]   -> levanta el servidor
//!   mini-redis cli [addr] CMD args...     -> ejecuta un comando
//!   mini-redis cli [addr]                 -> REPL interactivo
use mini_redis::{client, server};
use std::io::BufRead;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("server") => {
            let addr = args.get(1).cloned().unwrap_or_else(|| "127.0.0.1:6380".into());
            let snap = args.get(2).map(std::path::PathBuf::from);
            let shared = server::Shared::new(snap)?;
            let l = std::net::TcpListener::bind(&addr)?;
            eprintln!("mini-redis escuchando en {addr}");
            server::serve(l, shared);
            Ok(())
        }
        Some("cli") => {
            let addr = args.get(1).cloned().unwrap_or_else(|| "127.0.0.1:6380".into());
            let mut c = client::Client::connect(&addr)?;
            if args.len() > 2 {
                let cmd: Vec<&str> = args[2..].iter().map(String::as_str).collect();
                println!("{}", client::pretty(&c.cmd(&cmd)?));
                return Ok(());
            }
            // REPL: cada línea es un comando separado por espacios.
            for line in std::io::stdin().lock().lines() {
                let line = line?;
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }
                println!("{}", client::pretty(&c.cmd(&parts)?));
            }
            Ok(())
        }
        _ => {
            eprintln!("uso: mini-redis server [addr] [snapshot] | cli [addr] [CMD ...]");
            Ok(())
        }
    }
}
