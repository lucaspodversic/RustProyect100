//! # Reloj despertador para Raspberry Pi Pico
//!
//! Hardware objetivo: RP2040 + pantalla OLED SSD1306 128×64 (I2C) + tres
//! botones (MODO, MÁS, SNOOZE/MENOS) + un buzzer activo.
//!
//! ## Módulos
//! - [`hora`]: tipo `Hora` con aritmética circular de 24 h.
//! - [`botones`]: antirrebote por integración y detección de pulsación corta/larga.
//! - [`despertador`]: máquina de estados (normal, ajuste, sonando, pospuesta).
//! - [`fuente`]: fuente bitmap 5×7 (estilo clásico de LCD).
//! - [`pantalla`]: framebuffer 1 bpp con texto escalable y driver SSD1306.
//! - [`interfaz`]: dibuja la pantalla según el estado del despertador.
#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod botones;
pub mod despertador;
pub mod fuente;
pub mod hora;
pub mod interfaz;
pub mod pantalla;
