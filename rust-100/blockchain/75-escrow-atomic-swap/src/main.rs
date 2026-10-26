//! Demo del swap feliz: 1 BTC-sim ⇄ 15 ETH-sim.
use escrow_atomic_swap::cadena::Cadena;
use escrow_atomic_swap::swap::*;

fn main() {
    let mut a = Cadena::nueva("Cadena A", &[("alicia", 10), ("beto", 0)]);
    let mut b = Cadena::nueva("Cadena B", &[("alicia", 0), ("beto", 100)]);
    let ac = Acuerdo { alicia: "alicia".into(), beto: "beto".into(), monto_a: 1, monto_b: 15, t1: 48, t2: 24, margen: 6 };
    let alicia = Alicia::nueva();
    println!("H = {}", hex::encode(alicia.hashlock));
    let id_a = alicia.bloquear_en_a(&mut a, &ac).unwrap();
    let id_b = beto_verificar_y_bloquear(&a, id_a, &mut b, &ac).unwrap();
    a.avanzar(3);
    b.avanzar(3);
    alicia.reclamar_en_b(&mut b, id_b, &ac).unwrap();
    println!("Secreto revelado en B: {}", hex::encode(b.htlcs[id_b].preimagen.as_ref().unwrap()));
    beto_reclamar_en_a(&mut a, id_a, &b, id_b, &ac).unwrap();
    println!("A: alicia={} beto={} | B: alicia={} beto={}", a.saldo("alicia"), a.saldo("beto"), b.saldo("alicia"), b.saldo("beto"));
}
