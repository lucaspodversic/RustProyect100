//! Binario `mired`: muestra interfaces, IPs, gateway y DNS. `--json` para scripts.

use auditor_red::{direcciones, dns, gateway, puerto_abierto};
use clap::Parser;
use std::net::IpAddr;
use std::process::ExitCode;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "mired", version, about = "Auditor de red local: interfaces, IPs, gateway, DNS")]
struct Args {
    /// Salida JSON.
    #[arg(long)]
    json: bool,
    /// Ocultar loopback.
    #[arg(long)]
    sin_loopback: bool,
    /// Chequea puertos TCP en el gateway (ej. 22,53,80,443). Usalo solo en tu red.
    #[arg(long, value_delimiter = ',')]
    puertos: Vec<u16>,
}

fn main() -> ExitCode {
    let a = Args::parse();
    let dirs = match direcciones() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("no se pudieron leer interfaces: {e}");
            return ExitCode::from(1);
        }
    };
    let dirs: Vec<_> = dirs.into_iter().filter(|d| !(a.sin_loopback && d.tipo == auditor_red::TipoIp::Loopback)).collect();
    let gw = gateway();
    let dns = dns();
    if a.json {
        let v = serde_json::json!({
            "direcciones": dirs,
            "gateway": gw.as_ref().map(|(i, ip)| serde_json::json!({"interfaz": i, "ip": ip})),
            "dns": dns,
        });
        println!("{}", serde_json::to_string_pretty(&v).expect("json"));
        return ExitCode::SUCCESS;
    }
    println!("== Interfaces ==");
    for d in &dirs {
        let extra = match (&d.subred, d.prefijo_v6) {
            (Some(s), _) => format!("/{}  red {}  bcast {}  hosts {}", s.prefijo, s.red, s.broadcast, s.hosts),
            (None, Some(p)) => format!("/{p}"),
            _ => String::new(),
        };
        println!("{:<12} {:<40} {:<14} {}", d.interfaz, d.ip.to_string(), format!("{:?}", d.tipo), extra);
    }
    println!("\n== Gateway por defecto ==");
    match &gw {
        Some((i, ip)) => println!("{ip} (vía {i})"),
        None => println!("(no detectado)"),
    }
    println!("\n== DNS ==");
    if dns.is_empty() {
        println!("(no detectado)");
    }
    for d in &dns {
        println!("{d}");
    }
    if let (Some((_, ip)), false) = (&gw, a.puertos.is_empty()) {
        println!("\n== Puertos TCP en el gateway ==");
        for p in &a.puertos {
            let ok = puerto_abierto(IpAddr::V4(*ip), *p, Duration::from_millis(800));
            println!("{p:>5}  {}", if ok { "abierto" } else { "cerrado/filtrado" });
        }
    }
    ExitCode::SUCCESS
}
