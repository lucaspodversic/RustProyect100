//! Benchmark: `cargo bench` (o `cargo bench -- 5000000` para elegir n).
//!
//! Para cada algoritmo se toma la **mediana de 5 repeticiones** (más robusta
//! que la media frente a interrupciones del sistema operativo). Cada
//! repetición ordena una copia nueva de la misma entrada.

use ordenamiento_paralelo::datos::{generar, Distribucion};
use ordenamiento_paralelo::paralelo::{con_hilos, merge_sort_par, quicksort_par};
use ordenamiento_paralelo::secuencial::{merge_sort, quicksort};
use rayon::slice::ParallelSliceMut;
use std::time::Instant;

fn mediana_ms(base: &[u64], f: &dyn Fn(&mut [u64])) -> f64 {
    let mut tiempos: Vec<f64> = (0..5)
        .map(|_| {
            let mut v = base.to_vec();
            let t = Instant::now();
            f(&mut v);
            let ms = t.elapsed().as_secs_f64() * 1e3;
            assert!(ordenamiento_paralelo::esta_ordenado(&v));
            ms
        })
        .collect();
    tiempos.sort_by(f64::total_cmp);
    tiempos[2]
}

fn main() {
    let n: usize = std::env::args().filter_map(|a| a.parse().ok()).next().unwrap_or(2_000_000);
    let hilos = rayon::current_num_threads();
    println!("n = {n}, hilos de rayon = {hilos}\n");
    let algoritmos: Vec<(&str, Box<dyn Fn(&mut [u64])>)> = vec![
        ("std sort (estable)", Box::new(|v| v.sort())),
        ("std sort_unstable", Box::new(|v| v.sort_unstable())),
        ("merge_sort (sec)", Box::new(merge_sort)),
        ("quicksort (sec)", Box::new(quicksort)),
        ("merge_sort_par", Box::new(merge_sort_par)),
        ("quicksort_par", Box::new(quicksort_par)),
        ("rayon par_sort", Box::new(|v| v.par_sort())),
        ("rayon par_sort_unstable", Box::new(|v| v.par_sort_unstable())),
    ];
    for d in [Distribucion::Aleatoria, Distribucion::Ordenada, Distribucion::PocasClaves] {
        let base = generar(n, d, 2026);
        println!("── {d:?} ──");
        let referencia = mediana_ms(&base, &quicksort);
        for (nombre, f) in &algoritmos {
            let ms = mediana_ms(&base, f.as_ref());
            println!("  {nombre:<24} {ms:>9.2} ms   speedup vs quicksort sec: {:>5.2}x", referencia / ms);
        }
    }
    println!("\n── Escalado de quicksort_par (Aleatoria) ──");
    let base = generar(n, Distribucion::Aleatoria, 7);
    let t1 = con_hilos(1, || mediana_ms(&base, &quicksort_par));
    let mut h = 1;
    while h <= hilos.max(1) {
        let t = con_hilos(h, || mediana_ms(&base, &quicksort_par));
        println!("  {h:>2} hilos: {t:>9.2} ms  (speedup {:.2}x)", t1 / t);
        h *= 2;
    }
}
