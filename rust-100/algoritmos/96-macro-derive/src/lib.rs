//! # describir — un trait con *derive* propio
//!
//! ```
//! use describir::Describir;
//!
//! #[derive(Describir)]
//! struct Usuario {
//!     nombre: String,
//!     #[describir(renombrar = "años")]
//!     edad: u8,
//!     #[describir(omitir)]
//!     clave: String,
//! }
//!
//! let u = Usuario { nombre: "Ana".into(), edad: 30, clave: "s3cr3t".into() };
//! assert_eq!(u.describir(), r#"Usuario { nombre: "Ana", años: 30, .. }"#);
//! assert_eq!(Usuario::campos(), &["nombre", "años"]);
//! ```
//!
//! Errores de compilación que la macro genera a propósito (doctests
//! `compile_fail`, que pasan si el código NO compila):
//!
//! ```compile_fail
//! use describir::Describir;
//! #[derive(Describir)]
//! struct X { #[describir(ocultar)] a: u8 } // atributo desconocido
//! ```
//!
//! ```compile_fail
//! use describir::Describir;
//! #[derive(Describir)]
//! union U { a: u8, b: u16 } // las unions no se admiten
//! ```
//!
//! ```compile_fail
//! use describir::Describir;
//! struct SinDebug;
//! #[derive(Describir)]
//! struct Y { a: SinDebug } // los campos visibles necesitan Debug
//! ```
#![deny(missing_docs)]
#![forbid(unsafe_code)]

// Permite que `::describir::Describir` (la ruta que genera la macro) se
// resuelva también DENTRO de este crate (tests unitarios, ejemplos internos).
extern crate self as describir;

/// Reexportamos la macro con el mismo nombre que el trait: viven en
/// espacios de nombres distintos (macros vs tipos), igual que `Debug`.
pub use describir_derive::Describir;

/// Descripción legible de un valor, con control por atributos.
pub trait Describir {
    /// Nombre del tipo (o el indicado con `#[describir(nombre = "...")]`).
    fn nombre_tipo() -> &'static str
    where
        Self: Sized;
    /// Campos visibles (structs) o nombres de variantes (enums).
    fn campos() -> &'static [&'static str]
    where
        Self: Sized;
    /// Texto descriptivo del valor.
    fn describir(&self) -> String;
}

/// Utilidad genérica: describe una lista de elementos, uno por línea.
/// Funciona también con `dyn Describir` gracias a `?Sized`.
pub fn describir_todos<T: Describir + ?Sized>(xs: &[&T]) -> String {
    xs.iter().map(|x| x.describir()).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Describir)]
    struct Punto(i32, i32);

    #[test]
    fn tupla_dentro_del_crate() {
        assert_eq!(Punto(1, -2).describir(), "Punto(1, -2)");
        assert_eq!(Punto::campos(), &["0", "1"]);
    }
}
