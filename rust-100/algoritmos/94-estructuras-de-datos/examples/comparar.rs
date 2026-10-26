//! Compara tiempos de nuestras estructuras contra las de la std con 200 000
//! claves aleatorias. Ejecutar en release para que la medición tenga sentido:
//! `cargo run --release --example comparar`
use estructuras_de_datos::xorshift::XorShift;
use estructuras_de_datos::{Arbol, TablaHash};
use std::collections::{BTreeMap, HashMap};
use std::hint::black_box;
use std::time::Instant;

fn medir<R>(nombre: &str, f: impl FnOnce() -> R) -> R {
    let t = Instant::now();
    let r = black_box(f());
    println!("{nombre:<34} {:>8.2} ms", t.elapsed().as_secs_f64() * 1e3);
    r
}

fn main() {
    const N: usize = 200_000;
    let mut rng = XorShift::new(1);
    let claves: Vec<u64> = (0..N).map(|_| rng.siguiente()).collect();

    let t = medir("TablaHash (FNV-1a) insertar", || {
        let mut t = TablaHash::new();
        claves.iter().for_each(|&k| { t.insertar(k, k); });
        t
    });
    medir("TablaHash buscar", || black_box(&claves).iter().filter(|k| t.obtener(k).is_some()).count());
    let h = medir("std HashMap (SipHash) insertar", || claves.iter().map(|&k| (k, k)).collect::<HashMap<_, _>>());
    medir("std HashMap buscar", || black_box(&claves).iter().filter(|k| h.contains_key(k)).count());

    let a = medir("Arbol BST insertar (aleatorio)", || {
        let mut a = Arbol::new();
        claves.iter().for_each(|&k| { a.insertar(k, k); });
        a
    });
    medir("Arbol BST buscar", || black_box(&claves).iter().filter(|k| a.contiene(k)).count());
    println!("  altura del BST: {} (ideal ≈ log2 n = {:.0})", a.altura(), (N as f64).log2());
    let b = medir("std BTreeMap insertar", || claves.iter().map(|&k| (k, k)).collect::<BTreeMap<_, _>>());
    medir("std BTreeMap buscar", || black_box(&claves).iter().filter(|k| b.contains_key(k)).count());
    println!("cadena más larga en TablaHash: {} (capacidad {})", t.cadena_maxima(), t.capacidad());
}
