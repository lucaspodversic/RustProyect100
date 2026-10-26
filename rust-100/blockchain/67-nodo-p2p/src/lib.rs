//! # Nodo P2P de blockchain (proyecto 67)
//!
//! Arquitectura en capas:
//! - [`cadena`]: bloques con PoW y validación (heredado del proyecto 66).
//! - [`protocolo`]: mensajes JSON por línea (`Hola`, `PedirBloques`, …).
//! - [`nodo`]: máquina de estados pura (sin I/O) → fácil de testear.
//! - [`red`]: transporte TCP real en `127.0.0.1` con hilos.

pub mod cadena;
pub mod nodo;
pub mod protocolo;
pub mod red;

pub use cadena::Bloque;
pub use nodo::{sincronizar_en_memoria, Nodo, Recepcion};
pub use protocolo::Mensaje;
