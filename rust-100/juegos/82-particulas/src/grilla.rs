//! Grilla espacial dispersa ("spatial hash") para encontrar pares cercanos
//! en ~O(n) en vez de O(n²). Cada celda mide al menos el diámetro de
//! interacción, así basta mirar la celda propia y las 8 vecinas.
//!
//! Usamos un `HashMap` de celdas ocupadas: el costo depende de la cantidad
//! de partículas y no del tamaño del mundo (un mundo de 1000×1000 con
//! celdas de 0.5 tendría 4 millones de celdas densas).

use crate::vec2::V2;
use std::collections::HashMap;

pub struct Grilla {
    pub celda: f32,
    celdas: HashMap<(i32, i32), Vec<usize>>,
}

impl Grilla {
    pub fn new(celda: f32) -> Self {
        Self { celda, celdas: HashMap::new() }
    }

    fn coord(&self, p: V2) -> (i32, i32) {
        ((p.x / self.celda).floor() as i32, (p.y / self.celda).floor() as i32)
    }

    pub fn reconstruir(&mut self, posiciones: impl Iterator<Item = V2>) {
        // Vaciamos sin liberar memoria para reutilizar los Vec entre pasos.
        self.celdas.values_mut().for_each(Vec::clear);
        for (i, p) in posiciones.enumerate() {
            let c = self.coord(p);
            self.celdas.entry(c).or_default().push(i);
        }
        self.celdas.retain(|_, v| !v.is_empty());
    }

    /// Pares (i<j) a distancia < `radio` (requiere `radio <= celda`).
    pub fn pares(&self, pos: &[V2], radio: f32) -> Vec<(usize, usize)> {
        debug_assert!(radio <= self.celda + 1e-6);
        let mut out = Vec::new();
        let r2 = radio * radio;
        for (&(cx, cy), lista) in &self.celdas {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let Some(vecinas) = self.celdas.get(&(cx + dx, cy + dy)) else { continue };
                    for &i in lista {
                        for &j in vecinas {
                            if j > i && (pos[i] - pos[j]).len2() < r2 {
                                out.push((i, j));
                            }
                        }
                    }
                }
            }
        }
        out
    }
}
