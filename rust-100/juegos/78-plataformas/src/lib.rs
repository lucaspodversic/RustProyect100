//! # Juego de plataformas (proyecto 78)
//!
//! - [`nivel`]: tilemaps ASCII.
//! - [`fisica`]: gravedad, colisiones por ejes, coyote time, jump buffer, salto variable.
//! - [`juego`]: reglas, vidas, cámara y render a texto (headless).

pub mod fisica;
pub mod juego;
pub mod nivel;
