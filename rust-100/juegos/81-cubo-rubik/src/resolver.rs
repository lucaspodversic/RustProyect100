//! Mezclas aleatorias y un resolvedor por **IDDFS** (búsqueda en
//! profundidad iterativa) para mezclas cortas. Resolver un cubo arbitrario
//! óptimamente requiere tablas de poda (Kociemba/Korf); acá buscamos hasta
//! una profundidad máxima y podamos movimientos redundantes.

use crate::cubo::{Cubo, Eje, Mov};

/// Generador xorshift determinista (sin dependencias).
pub struct Rng(pub u64);

impl Rng {
    pub fn sig(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// ¿Es redundante hacer `m` después de `prev`? (misma cara, o cara opuesta
/// en orden "canónico" para no explorar R L y L R a la vez).
fn redundante(prev: Option<Mov>, m: Mov) -> bool {
    match prev {
        Some(p) => p.eje == m.eje || (p.eje.opuesto() == Some(m.eje) && (p.eje as u8) > (m.eje as u8)),
        None => false,
    }
}

/// Mezcla de `n` movimientos de cara sin repeticiones triviales.
pub fn mezcla(n: usize, semilla: u64) -> Vec<Mov> {
    let mut rng = Rng(semilla.max(1));
    let mut v: Vec<Mov> = Vec::with_capacity(n);
    while v.len() < n {
        let m = Mov { eje: Eje::CARAS[(rng.sig() % 6) as usize], giros: (rng.sig() % 3 + 1) as u8 };
        if !redundante(v.last().copied(), m) {
            v.push(m);
        }
    }
    v
}

/// Busca una solución de a lo sumo `max` movimientos (métrica HTM).
pub fn resolver(cubo: &Cubo, max: usize) -> Option<Vec<Mov>> {
    let mut camino = Vec::new();
    for prof in 0..=max {
        if dfs(&mut cubo.clone(), prof, &mut camino) {
            return Some(camino);
        }
    }
    None
}

fn dfs(c: &mut Cubo, restante: usize, camino: &mut Vec<Mov>) -> bool {
    if c.esta_resuelto() {
        return true;
    }
    if restante == 0 {
        return false;
    }
    for eje in Eje::CARAS {
        for giros in 1..=3 {
            let m = Mov { eje, giros };
            if redundante(camino.last().copied(), m) {
                continue;
            }
            let copia = c.clone();
            c.aplicar(m);
            camino.push(m);
            if dfs(c, restante - 1, camino) {
                return true;
            }
            camino.pop();
            *c = copia;
        }
    }
    false
}
