//! # Estación meteorológica IoT
//!
//! Este crate implementa la lógica completa de una estación meteorológica que
//! mide **temperatura** y **humedad relativa** con un sensor Sensirion SHT3x
//! (SHT30/SHT31/SHT35) conectado por I2C, agrega las lecturas y publica un
//! resumen en JSON a través de un transporte de red (Wi‑Fi en un ESP32).
//!
//! ## Organización
//! - [`sht3x`]: driver del sensor, genérico sobre `embedded_hal::i2c::I2c`.
//! - [`estadisticas`]: acumulador de mín/máx/promedio sin memoria dinámica.
//! - [`json`]: escritor de texto en un buffer fijo (`no_std`, sin `alloc`).
//! - [`red`]: trait [`red::Transporte`] + política de reintentos con backoff.
//! - [`estacion`]: orquestador que une todo en un ciclo `tick()`.
//!
//! ## ¿Por qué `no_std`?
//! En un microcontrolador no hay sistema operativo ni (necesariamente) heap.
//! Con `#![no_std]` nos obligamos a usar solo `core`, lo que garantiza que el
//! mismo código compila para `xtensa-esp32-none-elf`, `thumbv6m-none-eabi`
//! (RP2040), etc. Durante `cargo test` habilitamos `std` para poder usar los
//! mocks y el framework de tests.
#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod estacion;
pub mod estadisticas;
pub mod json;
pub mod red;
pub mod sht3x;
