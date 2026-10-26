//! # Monitor de calidad del aire
//!
//! Combina dos sensores muy usados en proyectos DIY:
//!
//! - **Plantower PMS5003** (UART 9600 8N1): material particulado PM1.0,
//!   PM2.5 y PM10 en µg/m³.
//! - **Sensirion SCD40** (I2C): CO₂ real por espectroscopía fotoacústica (ppm),
//!   más temperatura y humedad.
//!
//! Calcula el **índice AQI de la EPA** (revisión 2024) para PM2.5 y PM10,
//! clasifica el CO₂, muestra todo en un **LCD 16×2** y activa una alarma con
//! **histéresis** (para que no "aletee" cerca del umbral).
//!
//! ## Módulos
//! - [`pms5003`]: parser de tramas byte a byte con verificación de checksum.
//! - [`scd40`]: driver I2C del SCD40 (comandos de 16 bits + CRC‑8).
//! - [`aqi`]: tablas de cortes de la EPA y categorías.
//! - [`filtro`]: media móvil de ventana fija.
//! - [`lcd`]: driver HD44780 a través del expansor I2C PCF8574.
//! - [`monitor`]: integra todo y arma las dos líneas del display.
#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod aqi;
pub mod filtro;
pub mod lcd;
pub mod monitor;
pub mod pms5003;
pub mod scd40;
