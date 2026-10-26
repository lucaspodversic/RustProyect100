//! IA de demostración: BFS hacia la comida evitando el cuerpo; si no hay
//! camino, elige el movimiento seguro que deja más espacio libre (flood fill).

use crate::juego::{Dir, Modo, Pos, Snake};
use std::collections::{HashSet, VecDeque};

const DIRS: [Dir; 4] = [Dir::Arriba, Dir::Abajo, Dir::Izq, Dir::Der];

fn vecino(s: &Snake, p: Pos, d: Dir) -> Option<Pos> {
    let (dx, dy) = d.delta();
    let n = (p.0 + dx, p.1 + dy);
    if n.0 >= 0 && n.1 >= 0 && n.0 < s.ancho && n.1 < s.alto {
        Some(n)
    } else if s.modo == Modo::Toroide {
        Some((n.0.rem_euclid(s.ancho), n.1.rem_euclid(s.alto)))
    } else {
        None
    }
}

fn bloqueadas(s: &Snake) -> HashSet<Pos> {
    // La cola se libera en el próximo paso, por eso no la contamos.
    s.cuerpo.iter().take(s.cuerpo.len() - 1).copied().collect()
}

fn espacio(s: &Snake, desde: Pos, bloq: &HashSet<Pos>) -> usize {
    let mut vis = HashSet::from([desde]);
    let mut q = VecDeque::from([desde]);
    while let Some(p) = q.pop_front() {
        for d in DIRS {
            if let Some(n) = vecino(s, p, d) {
                if !bloq.contains(&n) && vis.insert(n) {
                    q.push_back(n);
                }
            }
        }
    }
    vis.len()
}

pub fn decidir(s: &Snake) -> Dir {
    let bloq = bloqueadas(s);
    let cabeza = s.cuerpo[0];
    let seguras: Vec<(Dir, Pos)> = DIRS.iter().filter(|d| **d != s.dir.opuesta()).filter_map(|d| vecino(s, cabeza, *d).map(|p| (*d, p))).filter(|(_, p)| !bloq.contains(p)).collect();
    // BFS desde cada vecino seguro hacia la comida; tomamos la primera dirección del camino más corto.
    if let Some(objetivo) = s.comida {
        let mut vis: HashSet<Pos> = HashSet::from([cabeza]);
        let mut q: VecDeque<(Pos, Dir)> = VecDeque::new();
        for (d, p) in &seguras {
            vis.insert(*p);
            q.push_back((*p, *d));
        }
        while let Some((p, primera)) = q.pop_front() {
            if p == objetivo {
                // Evitamos meternos en un callejón más chico que el cuerpo.
                let destino = vecino(s, cabeza, primera).unwrap();
                if espacio(s, destino, &bloq) > s.cuerpo.len() {
                    return primera;
                }
                break;
            }
            for d in DIRS {
                if let Some(n) = vecino(s, p, d) {
                    if !bloq.contains(&n) && vis.insert(n) {
                        q.push_back((n, primera));
                    }
                }
            }
        }
    }
    seguras.iter().max_by_key(|(_, p)| espacio(s, *p, &bloq)).map(|(d, _)| *d).unwrap_or(s.dir)
}
