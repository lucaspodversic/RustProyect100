//! # Riego automático por humedad del suelo
//!
//! Controla **N zonas** de riego (cada una con su sensor de humedad capacitivo
//! y su electroválvula) que comparten **una bomba**. Reglas principales:
//!
//! - Se riega una zona cuando su humedad baja de `umbral_bajo` y se corta al
//!   superar `umbral_alto` (**histéresis**) o al cumplirse `max_riego_ms`.
//! - Tras regar hay un **remojo** (*soak*): una pausa para que el agua
//!   penetre antes de volver a medir; si no, el sensor cercano al gotero
//!   "ve" humedad enseguida y corta antes de tiempo.
//! - Nunca se riega: si llueve, si el tanque está vacío (protege la bomba),
//!   fuera de la franja horaria permitida (evitar el mediodía por evaporación),
//!   o si la lectura del sensor es imposible (falla segura).
//! - **Una sola válvula abierta a la vez** para mantener la presión.
//!
//! ## Módulos
//! - [`sensor`]: calibración del sensor capacitivo y filtro de mediana.
//! - [`zona`]: estado de cada zona.
//! - [`controlador`]: planificador que decide qué zona riega.
#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod controlador;
pub mod sensor;
pub mod zona;
