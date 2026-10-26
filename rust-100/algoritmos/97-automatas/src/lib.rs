//! # Autómatas
//!
//! - [`turing`]: máquina de Turing determinista de una cinta, que se carga
//!   desde un formato de texto sencillo y se ejecuta con traza y límite de pasos.
//! - [`finito`]: autómatas finitos deterministas (AFD) y no deterministas
//!   con ε (AFN), conversión AFN→AFD por subconjuntos y minimización de AFD.
//!
//! Ambos modelos forman la jerarquía clásica de la computabilidad. Un AFD
//! reconoce lenguajes **regulares** con memoria finita. Una MT, gracias a su
//! cinta infinita, reconoce todo lo computable, pero a cambio puede **no
//! detenerse nunca** (problema de la parada). Por eso [`turing::Maquina::ejecutar`]
//! necesita un límite de pasos.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod finito;
pub mod turing;
