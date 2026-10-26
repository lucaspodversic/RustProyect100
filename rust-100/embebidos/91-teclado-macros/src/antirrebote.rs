//! Antirrebote por tecla.
//!
//! Un contacto mecánico rebota entre 1 y 5 ms al cambiar de estado. Por cada
//! tecla se guarda el estado **estable** y un contador. Un cambio se acepta
//! solo cuando la lectura cruda se mantiene distinta durante `UMBRAL` escaneos
//! seguidos. Con un escaneo por milisegundo y `UMBRAL = 5`, el retardo añadido
//! es de 5 ms.

use crate::matriz::Estado;

/// Escaneos consecutivos necesarios para aceptar un cambio.
pub const UMBRAL: u8 = 5;

/// Un evento de tecla ya filtrado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evento {
    /// La tecla (fila, columna) se pulsó.
    Pulsada(u8, u8),
    /// La tecla se soltó.
    Soltada(u8, u8),
}

/// Filtro de toda la matriz.
pub struct Antirrebote<const F: usize, const C: usize> {
    estable: Estado<F, C>,
    contador: [[u8; C]; F],
}

impl<const F: usize, const C: usize> Default for Antirrebote<F, C> {
    fn default() -> Self {
        Self { estable: [[false; C]; F], contador: [[0; C]; F] }
    }
}

impl<const F: usize, const C: usize> Antirrebote<F, C> {
    /// Procesa un escaneo crudo y llama a `evento` con cada cambio confirmado.
    /// Se usa un callback para no reservar memoria (sin `Vec` en `no_std`).
    pub fn actualizar(&mut self, crudo: &Estado<F, C>, mut evento: impl FnMut(Evento)) {
        for f in 0..F {
            for c in 0..C {
                if crudo[f][c] == self.estable[f][c] {
                    self.contador[f][c] = 0; // fue un rebote: se descarta
                    continue;
                }
                self.contador[f][c] += 1;
                if self.contador[f][c] >= UMBRAL {
                    self.contador[f][c] = 0;
                    self.estable[f][c] = crudo[f][c];
                    evento(if crudo[f][c] {
                        Evento::Pulsada(f as u8, c as u8)
                    } else {
                        Evento::Soltada(f as u8, c as u8)
                    });
                }
            }
        }
    }

    /// Estado estable actual.
    pub fn estable(&self) -> &Estado<F, C> {
        &self.estable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebote_se_ignora_y_cambio_estable_se_acepta() {
        let mut a = Antirrebote::<1, 1>::default();
        let mut ev = Vec::new();
        // Rebote: on, off, on, off → nada.
        for v in [true, false, true, false] {
            a.actualizar(&[[v]], |e| ev.push(e));
        }
        assert!(ev.is_empty());
        // 5 lecturas en 'pulsada' → un único evento.
        for _ in 0..8 {
            a.actualizar(&[[true]], |e| ev.push(e));
        }
        assert_eq!(ev, vec![Evento::Pulsada(0, 0)]);
        for _ in 0..5 {
            a.actualizar(&[[false]], |e| ev.push(e));
        }
        assert_eq!(ev[1], Evento::Soltada(0, 0));
    }
}
