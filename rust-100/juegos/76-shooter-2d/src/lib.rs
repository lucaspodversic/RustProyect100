//! # Shooter 2D (proyecto 76)
//!
//! Motor propio sin ventana ([`motor`]) + lógica de juego ([`juego`]).
//! Todo es testeable sin GPU: el render escribe en un `Framebuffer`.
//! Un frontend gráfico (macroquad/Bevy) sólo tendría que traducir teclas a
//! [`juego::Entrada`] y pintar el framebuffer o las entidades.

pub mod juego;
pub mod motor;
