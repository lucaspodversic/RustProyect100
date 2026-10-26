//! # Estructuras de datos desde cero
//!
//! Tres estructuras clásicas escritas en **Rust 100 % seguro** (sin `unsafe`):
//!
//! | Módulo | Estructura | Operaciones clave |
//! |---|---|---|
//! | [`lista`] | Lista simplemente enlazada (pila LIFO) | `push`/`pop` O(1), `invertir` O(n), iteradores |
//! | [`arbol`] | Árbol binario de búsqueda (BST) | insertar/buscar/eliminar O(h), recorrido en orden |
//! | [`tabla_hash`] | Tabla hash con encadenamiento + FNV‑1a | insertar/obtener/eliminar O(1) amortizado |
//!
//! Cada módulo explica sus decisiones de diseño y las trampas típicas del
//! *borrow checker* que aparecen al implementarlas.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod arbol;
pub mod lista;
pub mod tabla_hash;
pub mod xorshift;

pub use arbol::Arbol;
pub use lista::Lista;
pub use tabla_hash::TablaHash;
