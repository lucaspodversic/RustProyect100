//! Demo: construye un árbol con transacciones, genera y verifica una prueba.
use arbol_merkle::{verificar, ArbolMerkle, Modo};

fn main() {
    let txs: Vec<String> = std::env::args().skip(1).collect();
    let txs = if txs.is_empty() { (1..=7).map(|i| format!("tx{i}: A→B {i}0")).collect() } else { txs };
    let arbol = ArbolMerkle::desde_datos(Modo::Rfc6962, &txs);
    println!("{} hojas, {} niveles, raíz = {}", arbol.cantidad_hojas(), arbol.niveles.len(), hex::encode(arbol.raiz()));
    for (i, t) in txs.iter().enumerate() {
        let p = arbol.prueba(i).unwrap();
        let ok = verificar(Modo::Rfc6962, &Modo::Rfc6962.hash_hoja(t.as_bytes()), &p, &arbol.raiz());
        println!("  [{i}] {t:<16} prueba de {} hashes → {}", p.pasos.len(), if ok { "✔" } else { "✘" });
    }
}
