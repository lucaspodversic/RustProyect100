//! Demostración rápida: ordena 1 millón de números con cada algoritmo.
//! `cargo run --release --example demo`
use ordenamiento_paralelo::datos::{generar, Distribucion};
use ordenamiento_paralelo::paralelo::{merge_sort_par, quicksort_par};
use ordenamiento_paralelo::secuencial::{merge_sort, quicksort};
use std::time::Instant;

fn main() {
    let base = generar(1_000_000, Distribucion::Aleatoria, 1);
    for (nombre, f) in [
        ("merge_sort", merge_sort as fn(&mut [u64])),
        ("quicksort", quicksort),
        ("merge_sort_par", merge_sort_par),
        ("quicksort_par", quicksort_par),
    ] {
        let mut v = base.clone();
        let t = Instant::now();
        f(&mut v);
        println!("{nombre:<15} {:>7.2} ms  primeros: {:?}", t.elapsed().as_secs_f64() * 1e3, &v[..3]);
    }
}
