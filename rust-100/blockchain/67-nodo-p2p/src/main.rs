//! Demo: levanta tres nodos en localhost, mina en uno y sincroniza los demás.

use nodo_p2p::red::{difundir, sincronizar_con, Servidor};
use nodo_p2p::Nodo;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let a = Servidor::iniciar(Nodo::nuevo("A"))?;
    let b = Servidor::iniciar(Nodo::nuevo("B"))?;
    let c = Servidor::iniciar(Nodo::nuevo("C"))?;
    println!("Nodos en {} / {} / {}", a.direccion, b.direccion, c.direccion);

    // A mina 5 bloques.
    for i in 1..=5 {
        a.nodo.lock().unwrap().minar(format!("tx-{i}"), i);
    }
    println!("A altura={}", a.altura());

    // B se sincroniza contra A por TCP.
    sincronizar_con(&b.nodo, a.direccion, Some(b.direccion.to_string()))?;
    println!("B tras sincronizar con A: altura={}", b.altura());

    // B mina un bloque y lo difunde por gossip a A y C.
    let anuncio = b.nodo.lock().unwrap().minar("tx-6 desde B", 6);
    difundir(&anuncio, &[a.direccion, c.direccion]);
    std::thread::sleep(Duration::from_millis(100));
    println!("A tras gossip: altura={}  (C recibió un bloque adelantado: {} huérfanos)", a.altura(), c.nodo.lock().unwrap().huerfanos.len());

    // C se pone al día contra B.
    sincronizar_con(&c.nodo, b.direccion, None)?;
    println!("C tras sincronizar con B: altura={}", c.altura());
    Ok(())
}
