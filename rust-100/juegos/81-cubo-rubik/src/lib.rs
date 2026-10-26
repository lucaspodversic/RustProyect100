//! # Cubo de Rubik (proyecto 81)
//!
//! - [`geometria`]: deriva las permutaciones de cada giro rotando vectores 3D.
//! - [`cubo`]: estado de 54 stickers, notación Singmaster, inversa, orden, red.
//! - [`resolver`]: mezclas reproducibles y resolvedor IDDFS para mezclas cortas.

pub mod cubo;
pub mod geometria;
pub mod resolver;
