//! # Laberintos
//!
//! - [`rejilla`]: representación, parseo desde texto y dibujo.
//! - [`generador`]: laberintos perfectos aleatorios (backtracker) con
//!   opción de abrir ciclos y añadir barro.
//! - [`busqueda`]: BFS, DFS, Dijkstra y A*, con estadísticas de nodos expandidos.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod busqueda;
pub mod generador;
pub mod rejilla;
