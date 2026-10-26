//! # Benchmark de concurrencia
//!
//! Este crate implementa **la misma tarea** con distintos modelos de
//! concurrencia y comprueba que todos dan el mismo resultado.
//!
//! | Modelo | Unidad | Ideal para |
//! |---|---|---|
//! | `std::thread` | hilo del SO (~8 MiB de pila virtual y una llamada al sistema para crearlo) | pocos trabajos largos, control total |
//! | rayon | tareas en un pool fijo con *work stealing* | **CPU**: paralelismo de datos |
//! | tokio | *futures* en un *runtime* asíncrono | **I/O**: miles de esperas concurrentes |
//!
//! Regla práctica: **rayon para calcular, tokio para esperar**. Mezclarlos
//! mal (por ejemplo, cálculo pesado dentro de una tarea async) bloquea los
//! hilos del runtime. Para eso existe `spawn_blocking`.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod cpu;
pub mod io;

/// Mide el tiempo de ejecución de `f` y devuelve también su resultado.
pub fn medir<R>(f: impl FnOnce() -> R) -> (R, std::time::Duration) {
    let t = std::time::Instant::now();
    let r = f();
    (r, t.elapsed())
}

/// Número de hilos hardware disponibles (mínimo 1).
pub fn nucleos() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}
