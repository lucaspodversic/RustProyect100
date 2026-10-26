//! # Controlador de luces ambientales
//!
//! Controla una tira LED RGB (tres canales PWM) eligiendo el color en función de:
//!
//! 1. **La hora del día** — una "curva circadiana" de fotogramas clave
//!    (cálido al amanecer, blanco frío al mediodía, ámbar al atardecer,
//!    rojo tenue de noche para no afectar la melatonina).
//! 2. **La luz ambiente** — si ya hay mucha luz natural, se baja el brillo.
//! 3. **La presencia** — un sensor PIR; sin movimiento durante un tiempo
//!    configurable, la luz se apaga con un fundido suave.
//!
//! Todo en aritmética entera y `no_std`.
//!
//! ## Módulos
//! - [`color`]: tipos `Rgb`/`Hsv`, conversión, interpolación y corrección gamma.
//! - [`horario`]: curva de fotogramas clave por minuto del día.
//! - [`sensores`]: trait de sensor analógico y conversión a lux.
//! - [`controlador`]: máquina que decide el color objetivo y hace el fundido.
#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod color;
pub mod controlador;
pub mod horario;
pub mod sensores;
