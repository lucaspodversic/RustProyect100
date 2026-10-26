//! # Snake con high score persistente (proyecto 77)
//!
//! - [`juego`]: lógica pura (determinista, sin E/S).
//! - [`ia`]: piloto automático BFS + flood fill (para demo y tests).
//! - [`records`]: tabla top-10 en JSON con escritura atómica.

pub mod ia;
pub mod juego;
pub mod records;
