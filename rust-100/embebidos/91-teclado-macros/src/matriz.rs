//! Escaneo de una matriz de teclas.
//!
//! En una matriz cada tecla une una **fila** con una **columna**, con un
//! diodo en serie para evitar las "teclas fantasma" (*ghosting*). El escaneo
//! funciona así:
//!
//! 1. Todas las filas quedan en alto. Las columnas son entradas con pull‑up.
//! 2. Se pone en bajo una fila a la vez.
//! 3. Cada columna que lee **bajo** corresponde a una tecla pulsada de esa fila.
//! 4. La fila vuelve a alto y se pasa a la siguiente.
//!
//! Así, `F + C` pines sirven para leer `F × C` teclas.

use embedded_hal::digital::{InputPin, OutputPin};

/// Estado crudo de la matriz: `true` = pulsada.
pub type Estado<const F: usize, const C: usize> = [[bool; C]; F];

/// Matriz de `F` filas y `C` columnas.
pub struct Matriz<O, I, const F: usize, const C: usize> {
    filas: [O; F],
    columnas: [I; C],
}

impl<O: OutputPin, I: InputPin, const F: usize, const C: usize> Matriz<O, I, F, C> {
    /// Crea la matriz y deja todas las filas inactivas (alto).
    pub fn new(mut filas: [O; F], columnas: [I; C]) -> Self {
        for f in filas.iter_mut() {
            let _ = f.set_high();
        }
        Self { filas, columnas }
    }

    /// Escanea la matriz completa. En hardware real conviene esperar ~1 µs
    /// tras activar cada fila para que la línea se estabilice. Para eso se
    /// recibe `asentar`, que puede ser un `delay_us(1)` o no hacer nada.
    pub fn escanear(&mut self, mut asentar: impl FnMut()) -> Estado<F, C> {
        let mut e = [[false; C]; F];
        for (f, fila) in self.filas.iter_mut().enumerate() {
            let _ = fila.set_low();
            asentar();
            for (c, col) in self.columnas.iter_mut().enumerate() {
                // Si la lectura falla, se toma como "no pulsada" (lo más seguro).
                e[f][c] = col.is_low().unwrap_or(false);
            }
            let _ = fila.set_high();
        }
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::digital::{Mock, State, Transaction as T};

    #[test]
    fn escaneo_2x2_con_mocks() {
        // Fila 0: activar, leer cols (col1 pulsada), desactivar.
        let f0 = Mock::new(&[T::set(State::High), T::set(State::Low), T::set(State::High)]);
        let f1 = Mock::new(&[T::set(State::High), T::set(State::Low), T::set(State::High)]);
        // Columnas: se leen una vez por fila (alto = suelta, bajo = pulsada).
        let c0 = Mock::new(&[T::get(State::High), T::get(State::Low)]);
        let c1 = Mock::new(&[T::get(State::Low), T::get(State::High)]);
        let mut m = Matriz::new([f0.clone(), f1.clone()], [c0.clone(), c1.clone()]);
        let e = m.escanear(|| {});
        assert_eq!(e, [[false, true], [true, false]]);
        for mut p in [f0, f1, c0, c1] {
            p.done();
        }
    }
}
