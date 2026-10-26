//! # Teclado de macros (macropad) para RP2040
//!
//! Esta es la lógica completa de un macropad de 3×3 teclas. No depende de
//! ningún hardware concreto:
//!
//! ```text
//!  pines ──► [matriz] ──► [antirrebote] ──► [teclado: capas + macros] ──► [hid] ──► USB
//! ```
//!
//! - [`matriz`]: escanea filas y columnas con `OutputPin`/`InputPin`.
//! - [`antirrebote`]: un contador por tecla filtra los rebotes mecánicos.
//! - [`mapa`]: acciones por tecla y por capa (tecla, modificador, capa, macro).
//! - [`macros`]: secuencias de pasos (pulsar, soltar, texto, esperar).
//! - [`hid`]: códigos de uso HID y reporte de 8 bytes con 6KRO.
//! - [`teclado`]: une todo y produce un reporte por tick.
#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod antirrebote;
pub mod hid;
pub mod macros;
pub mod mapa;
pub mod matriz;
pub mod teclado;
