//! # Driver del IMU MPU‑6050
//!
//! El MPU‑6050 (InvenSense/TDK) combina un **acelerómetro** y un
//! **giroscopio** de 3 ejes, más un sensor de temperatura, y se comunica por
//! I2C (dirección 0x68, o 0x69 si el pin AD0 está en alto).
//!
//! Este driver:
//! - es **genérico** sobre `embedded_hal::i2c::I2c` y `delay::DelayNs`, por
//!   lo que funciona igual en ESP32, RP2040 o STM32;
//! - no usa memoria dinámica ni punto flotante en la ruta de lectura (los
//!   datos crudos son `i16` y la conversión a unidades físicas es opcional);
//! - verifica la identidad del chip (`WHO_AM_I`);
//! - permite elegir los rangos de escala y el filtro paso bajo (DLPF);
//! - lee los 14 bytes de datos en **una sola transacción** (*burst*) para que
//!   todos los ejes correspondan al mismo instante;
//! - calibra el *bias* del giroscopio.
//!
//! ## Módulos
//! - [`registros`]: mapa de registros y valores de configuración.
//! - [`mpu6050`]: el driver.
#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod mpu6050;
pub mod registros;

pub use mpu6050::{Datos, Error, Mpu6050};
pub use registros::{Dlpf, RangoAcel, RangoGiro};
