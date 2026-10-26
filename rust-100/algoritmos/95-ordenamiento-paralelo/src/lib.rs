//! # Ordenamiento paralelo con rayon
//!
//! Los algoritmos de **divide y vencerás** se paralelizan de forma natural:
//! las dos mitades son independientes. `rayon::join(a, b)` ejecuta `a` y
//! `b` potencialmente en paralelo usando *work stealing*: si hay hilos
//! libres, uno "roba" `b`; si no, lo ejecuta el mismo hilo sin coste extra
//! de sincronización.
//!
//! El problema típico es que crear tareas para trozos diminutos cuesta más
//! de lo que ahorra. Por eso todos los algoritmos usan un **umbral**: por
//! debajo de cierto tamaño se ordena de forma secuencial (y por debajo de 32
//! elementos se usa inserción, que es más rápida para listas pequeñas).
//!
//! - [`secuencial`]: inserción, merge sort y quicksort de referencia.
//! - [`paralelo`]: las versiones con `rayon::join`.
//! - [`datos`]: generadores de entradas (aleatoria, ordenada, inversa, pocas claves).
#![deny(missing_docs)]

pub mod datos;
pub mod paralelo;
pub mod secuencial;

/// Comprueba si un slice está ordenado (no decreciente).
pub fn esta_ordenado<T: Ord>(v: &[T]) -> bool {
    v.windows(2).all(|w| w[0] <= w[1])
}
