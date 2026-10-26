//! # Pong multijugador local por TCP (proyecto 84)
//!
//! - [`juego`]: física y reglas, puras y deterministas.
//! - [`protocolo`]: mensajes de texto por línea, con validación.
//! - [`red`]: servidor autoritativo con tick fijo y cliente.

pub mod juego;
pub mod protocolo;
pub mod red;
