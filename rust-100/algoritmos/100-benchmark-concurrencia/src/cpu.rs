//! Carga **CPU‑bound**: contar los primos de `[0, limite)` dividiéndolo en
//! `trozos` subrangos (las "tareas").
//!
//! Los primos se cuentan por división de prueba a propósito: es lento y
//! **desequilibrado**, porque los números grandes cuestan más. Así se nota
//! la diferencia entre repartir el trabajo de forma estática (un bloque por
//! hilo) y dinámica (cola de trabajo o *work stealing*).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

/// ¿Es primo? División de prueba por 6k±1 hasta √n.
pub fn es_primo(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n < 4 {
        return true;
    }
    if n % 2 == 0 || n % 3 == 0 {
        return false;
    }
    let mut i = 5;
    while i * i <= n {
        if n % i == 0 || n % (i + 2) == 0 {
            return false;
        }
        i += 6;
    }
    true
}

/// Cuenta los primos de un rango (una "tarea").
pub fn contar(r: std::ops::Range<u64>) -> u64 {
    r.filter(|&n| es_primo(n)).count() as u64
}

/// Divide `[0, limite)` en `n` subrangos contiguos.
pub fn tareas(limite: u64, n: usize) -> Vec<std::ops::Range<u64>> {
    let n = n.max(1) as u64;
    (0..n).map(|i| (limite * i / n)..(limite * (i + 1) / n)).collect()
}

/// Estrategias CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estrategia {
    /// Un solo hilo.
    Secuencial,
    /// `thread::scope`: un hilo por núcleo, bloques **estáticos** de tareas.
    HilosBloques,
    /// Un hilo del SO **por tarea** (muestra el coste de crear hilos).
    HiloPorTarea,
    /// Un hilo por núcleo + índice atómico compartido (cola **dinámica**).
    HilosColaAtomica,
    /// Hilos + canal `mpsc` para enviar los resultados (patrón productor/consumidor).
    HilosCanal,
    /// `rayon::par_iter` (work stealing).
    Rayon,
    /// tokio: cada tarea con `spawn_blocking` (pool de hilos bloqueantes).
    TokioSpawnBlocking,
    /// tokio: cada tarea con `spawn` normal, calculando dentro de la tarea async.
    /// Funciona, pero bloquea los *workers* del runtime: es un antipatrón
    /// si a la vez hubiera I/O.
    TokioSpawn,
}

impl Estrategia {
    /// Todas.
    pub const TODAS: [Estrategia; 8] = [
        Estrategia::Secuencial,
        Estrategia::HilosBloques,
        Estrategia::HiloPorTarea,
        Estrategia::HilosColaAtomica,
        Estrategia::HilosCanal,
        Estrategia::Rayon,
        Estrategia::TokioSpawnBlocking,
        Estrategia::TokioSpawn,
    ];
}

/// Ejecuta la estrategia `e` sobre `ts` con `hilos` hilos y devuelve el
/// total de primos.
pub fn ejecutar(e: Estrategia, ts: &[std::ops::Range<u64>], hilos: usize) -> u64 {
    let hilos = hilos.max(1);
    match e {
        Estrategia::Secuencial => ts.iter().cloned().map(contar).sum(),

        Estrategia::HilosBloques => {
            // `thread::scope` (Rust 1.63+) permite que los hilos tomen
            // prestado `ts` sin `Arc`: el scope garantiza que todos terminan
            // antes de que `ts` deje de existir.
            let tam = ts.len().div_ceil(hilos);
            std::thread::scope(|s| {
                let hs: Vec<_> = ts
                    .chunks(tam.max(1))
                    .map(|bloque| s.spawn(move || bloque.iter().cloned().map(contar).sum::<u64>()))
                    .collect();
                hs.into_iter().map(|h| h.join().expect("hilo")).sum()
            })
        }

        Estrategia::HiloPorTarea => std::thread::scope(|s| {
            let hs: Vec<_> = ts.iter().map(|r| s.spawn(move || contar(r.clone()))).collect();
            hs.into_iter().map(|h| h.join().expect("hilo")).sum()
        }),

        Estrategia::HilosColaAtomica => {
            // Cada hilo "toma un número" con fetch_add hasta agotar las
            // tareas. Los hilos rápidos cogen más tareas, así que el reparto
            // se equilibra solo. `Relaxed` basta porque solo se necesita que
            // el contador sea atómico, no ordenar otros accesos a memoria.
            let siguiente = AtomicUsize::new(0);
            std::thread::scope(|s| {
                let hs: Vec<_> = (0..hilos)
                    .map(|_| {
                        s.spawn(|| {
                            let mut suma = 0;
                            loop {
                                let i = siguiente.fetch_add(1, Ordering::Relaxed);
                                let Some(r) = ts.get(i) else { break suma };
                                suma += contar(r.clone());
                            }
                        })
                    })
                    .collect();
                hs.into_iter().map(|h| h.join().expect("hilo")).sum()
            })
        }

        Estrategia::HilosCanal => {
            let siguiente = AtomicUsize::new(0);
            let (tx, rx) = mpsc::channel::<u64>();
            std::thread::scope(|s| {
                for _ in 0..hilos {
                    let tx = tx.clone();
                    let siguiente = &siguiente;
                    s.spawn(move || {
                        while let Some(r) = ts.get(siguiente.fetch_add(1, Ordering::Relaxed)) {
                            tx.send(contar(r.clone())).expect("receptor vivo");
                        }
                    });
                }
                // Hay que soltar el emisor original: si no, `rx` nunca vería
                // el canal cerrado y el bucle `iter()` no terminaría.
                drop(tx);
                rx.iter().sum()
            })
        }

        Estrategia::Rayon => {
            use rayon::prelude::*;
            // `with_max_len(1)`: con pocas tareas muy desiguales, el reparto
            // adaptativo de rayon puede agrupar varias en un mismo trozo y
            // dejar al final un "rezagado". Forzar trozos de 1 tarea iguala
            // a la cola atómica (medido: 35 ms → 24 ms con 64 tareas).
            con_pool_rayon(hilos, || ts.par_iter().with_max_len(1).cloned().map(contar).sum())
        }

        Estrategia::TokioSpawnBlocking => runtime(hilos).block_on(async {
            let hs: Vec<_> = ts.iter().cloned().map(|r| tokio::task::spawn_blocking(move || contar(r))).collect();
            let mut total = 0;
            for h in hs {
                total += h.await.expect("tarea");
            }
            total
        }),

        Estrategia::TokioSpawn => runtime(hilos).block_on(async {
            let hs: Vec<_> = ts.iter().cloned().map(|r| tokio::spawn(async move { contar(r) })).collect();
            let mut total = 0;
            for h in hs {
                total += h.await.expect("tarea");
            }
            total
        }),
    }
}

/// Runtime tokio multihilo con `hilos` workers. `max_blocking_threads`
/// también se limita para que la comparación sea justa.
pub fn runtime(hilos: usize) -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(hilos)
        .max_blocking_threads(hilos)
        .enable_time()
        .build()
        .expect("runtime")
}

/// Ejecuta `f` dentro de un pool de rayon de `hilos` hilos.
pub fn con_pool_rayon<R: Send>(hilos: usize, f: impl FnOnce() -> R + Send) -> R {
    rayon::ThreadPoolBuilder::new().num_threads(hilos).build().expect("pool").install(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primos_conocidos() {
        assert_eq!(contar(0..100), 25);
        assert_eq!(contar(0..10_000), 1229);
        assert!(es_primo(1_000_000_007));
        assert!(!es_primo(1_000_000_007 * 3));
    }

    #[test]
    fn tareas_cubren_el_rango() {
        let ts = tareas(1000, 7);
        assert_eq!(ts.first().unwrap().start, 0);
        assert_eq!(ts.last().unwrap().end, 1000);
        assert!(ts.windows(2).all(|w| w[0].end == w[1].start));
    }
}
