//! # Reproductor de música en terminal (proyecto 83)
//!
//! - [`audio`]: trait del dispositivo + backend simulado.
//! - [`lista`]: pistas, M3U y formato de tiempo.
//! - [`reproductor`]: estados, cola, aleatorio, repetir, volumen.
//! - [`ui`]: interfaz ratatui (testeable con `TestBackend`).

pub mod audio;
pub mod lista;
pub mod reproductor;
pub mod ui;
