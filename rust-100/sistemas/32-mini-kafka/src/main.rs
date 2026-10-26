//! `mini-kafka [addr] [data_dir]` levanta el broker TCP.
//! Probar con: `nc 127.0.0.1 9092` y escribir `CREATE t 3`, `PRODUCE t k hola`...
use mini_kafka::{broker::Broker, net};
use std::sync::{Arc, Mutex};

fn main() -> std::io::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let addr = a.get(1).cloned().unwrap_or_else(|| "127.0.0.1:9092".into());
    let broker = match a.get(2) {
        Some(d) => Broker::with_dir(d.into())?,
        None => Broker::in_memory(),
    };
    let l = std::net::TcpListener::bind(&addr)?;
    eprintln!("mini-kafka escuchando en {addr}");
    net::serve(l, Arc::new(Mutex::new(broker)));
    Ok(())
}
