//! Carga **I/O‑bound** simulada: `n` "peticiones" que esperan `ms`
//! milisegundos cada una (como una llamada de red) y devuelven un valor.
//!
//! En este caso no hay nada que calcular: lo que importa es **cuántas
//! esperas pueden solaparse a la vez**.
//!
//! - Secuencial: n·ms.
//! - Un hilo del SO por petición: aproximadamente ms, pero cada hilo cuesta
//!   memoria y una llamada al sistema. Con 10 000 peticiones se nota, y con
//!   100 000 puede fallar.
//! - rayon: el pool tiene tantos hilos como núcleos y `sleep` **bloquea**
//!   el hilo. Por eso tarda ≈ n/núcleos·ms: es la herramienta equivocada.
//! - tokio: `tokio::time::sleep` no bloquea, solo registra un temporizador.
//!   Decenas de miles de esperas caben en pocos hilos y tardan ≈ ms.

use std::time::Duration;
use tokio::sync::Semaphore;

/// Estrategias I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstrategiaIo {
    /// Una tras otra.
    Secuencial,
    /// Un hilo del SO por petición, con `thread::sleep`.
    HiloPorPeticion,
    /// rayon con `thread::sleep` (antipatrón: bloquea el pool).
    Rayon,
    /// tokio con `tokio::time::sleep` y `JoinSet`.
    Tokio,
    /// tokio con un semáforo que limita la concurrencia (p. ej. a 64),
    /// como se haría para no saturar un servidor externo.
    TokioLimitado(usize),
}

/// "Petición" síncrona: espera y devuelve el doble del id.
fn peticion_bloqueante(id: u64, ms: u64) -> u64 {
    std::thread::sleep(Duration::from_millis(ms));
    id * 2
}

/// "Petición" asíncrona.
async fn peticion_async(id: u64, ms: u64) -> u64 {
    tokio::time::sleep(Duration::from_millis(ms)).await;
    id * 2
}

/// Ejecuta `n` peticiones de `ms` milisegundos. Devuelve la suma de las
/// respuestas (igual en todas las estrategias: n·(n−1)).
pub fn ejecutar(e: EstrategiaIo, n: u64, ms: u64, hilos: usize) -> u64 {
    match e {
        EstrategiaIo::Secuencial => (0..n).map(|i| peticion_bloqueante(i, ms)).sum(),
        EstrategiaIo::HiloPorPeticion => std::thread::scope(|s| {
            let hs: Vec<_> = (0..n)
                .map(|i| {
                    // Pila pequeña (64 KiB): solo duerme, no necesita más.
                    std::thread::Builder::new()
                        .stack_size(64 * 1024)
                        .spawn_scoped(s, move || peticion_bloqueante(i, ms))
                        .expect("crear hilo")
                })
                .collect();
            hs.into_iter().map(|h| h.join().expect("hilo")).sum()
        }),
        EstrategiaIo::Rayon => {
            use rayon::prelude::*;
            crate::cpu::con_pool_rayon(hilos, || (0..n).into_par_iter().map(|i| peticion_bloqueante(i, ms)).sum())
        }
        EstrategiaIo::Tokio => crate::cpu::runtime(hilos).block_on(async {
            // JoinSet: conjunto de tareas que se recogen según terminan.
            let mut set = tokio::task::JoinSet::new();
            for i in 0..n {
                set.spawn(peticion_async(i, ms));
            }
            let mut total = 0;
            while let Some(r) = set.join_next().await {
                total += r.expect("tarea");
            }
            total
        }),
        EstrategiaIo::TokioLimitado(max) => crate::cpu::runtime(hilos).block_on(async {
            let sem = std::sync::Arc::new(Semaphore::new(max.max(1)));
            let mut set = tokio::task::JoinSet::new();
            for i in 0..n {
                let sem = sem.clone();
                set.spawn(async move {
                    // El permiso se libera al soltarse `_p` (RAII).
                    let _p = sem.acquire_owned().await.expect("semáforo abierto");
                    peticion_async(i, ms).await
                });
            }
            let mut total = 0;
            while let Some(r) = set.join_next().await {
                total += r.expect("tarea");
            }
            total
        }),
    }
}
