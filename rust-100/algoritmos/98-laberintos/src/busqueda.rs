//! Algoritmos de búsqueda de caminos.
//!
//! | Algoritmo | Estructura | ¿Óptimo? |
//! |---|---|---|
//! | BFS | cola FIFO | en **número de pasos** (ignora el barro) |
//! | DFS | pila LIFO | no: devuelve el primer camino que encuentra |
//! | Dijkstra | montículo por coste g | en **coste** |
//! | A* | montículo por f = g + h | en coste, si h es admisible |
//!
//! La heurística de A* es la **distancia Manhattan**, que es admisible
//! porque nunca sobreestima: cada paso cuesta al menos 1 y solo se puede
//! mover en 4 direcciones. También es consistente, así que basta con
//! expandir cada nodo una vez.

use crate::rejilla::{Laberinto, Pos};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

/// Algoritmo a usar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algoritmo {
    /// Búsqueda en anchura.
    Bfs,
    /// Búsqueda en profundidad.
    Dfs,
    /// Dijkstra (A* con h = 0).
    Dijkstra,
    /// A* con distancia Manhattan.
    AEstrella,
}

/// Resultado de una búsqueda con éxito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Solucion {
    /// Camino de inicio a salida (ambos incluidos).
    pub camino: Vec<Pos>,
    /// Coste total (suma de costes de las casillas en las que se entra).
    pub coste: u32,
    /// Nodos expandidos: mide el "trabajo" del algoritmo.
    pub expandidos: usize,
}

/// Reconstruye el camino siguiendo los padres desde la salida.
fn reconstruir(l: &Laberinto, padre: &[usize], fin: Pos) -> Vec<Pos> {
    let mut camino = vec![fin];
    let mut i = l.indice(fin);
    while padre[i] != usize::MAX {
        i = padre[i];
        camino.push((i / l.ancho, i % l.ancho));
    }
    camino.reverse();
    camino
}

/// Coste de recorrer un camino (no cuenta la casilla inicial).
pub fn coste_camino(l: &Laberinto, camino: &[Pos]) -> u32 {
    camino.iter().skip(1).map(|&p| l.celda(p).coste().expect("camino por muro")).sum()
}

/// Comprueba que un camino es válido: empieza y termina bien, pasos
/// adyacentes y sin atravesar muros.
pub fn es_valido(l: &Laberinto, camino: &[Pos]) -> bool {
    camino.first() == Some(&l.inicio)
        && camino.last() == Some(&l.salida)
        && camino.windows(2).all(|w| l.vecinos(w[0]).any(|v| v == w[1]))
}

/// Resuelve con el algoritmo indicado. `None` si no hay camino.
pub fn resolver(l: &Laberinto, alg: Algoritmo) -> Option<Solucion> {
    match alg {
        Algoritmo::Bfs => bfs(l),
        Algoritmo::Dfs => dfs(l),
        Algoritmo::Dijkstra => mejor_primero(l, |_| 0),
        Algoritmo::AEstrella => {
            let (sf, sc) = l.salida;
            mejor_primero(l, move |(f, c): Pos| (f.abs_diff(sf) + c.abs_diff(sc)) as u32)
        }
    }
}

/// BFS: explora por "capas" de distancia. El primer momento en que se
/// alcanza la salida corresponde al camino con menos pasos.
fn bfs(l: &Laberinto) -> Option<Solucion> {
    let n = l.celdas.len();
    let mut padre = vec![usize::MAX; n];
    let mut visto = vec![false; n];
    let mut cola = VecDeque::from([l.inicio]);
    visto[l.indice(l.inicio)] = true;
    let mut expandidos = 0;
    while let Some(p) = cola.pop_front() {
        expandidos += 1;
        if p == l.salida {
            let camino = reconstruir(l, &padre, p);
            return Some(Solucion { coste: coste_camino(l, &camino), camino, expandidos });
        }
        for v in l.vecinos(p) {
            let iv = l.indice(v);
            // Marcamos al ENCOLAR (no al sacar): evita duplicados en la cola.
            if !visto[iv] {
                visto[iv] = true;
                padre[iv] = l.indice(p);
                cola.push_back(v);
            }
        }
    }
    None
}

/// DFS iterativa: igual que BFS pero con pila. Va "hasta el fondo" de un
/// pasillo antes de probar otro. Usa poca memoria en laberintos estrechos,
/// pero el camino puede ser larguísimo.
fn dfs(l: &Laberinto) -> Option<Solucion> {
    let n = l.celdas.len();
    let mut padre = vec![usize::MAX; n];
    let mut visto = vec![false; n];
    let mut pila = vec![l.inicio];
    let mut expandidos = 0;
    while let Some(p) = pila.pop() {
        let ip = l.indice(p);
        // Con pila se marca al SACAR: así el padre registrado es el del
        // camino que realmente se está explorando.
        if visto[ip] {
            continue;
        }
        visto[ip] = true;
        expandidos += 1;
        if p == l.salida {
            let camino = reconstruir(l, &padre, p);
            return Some(Solucion { coste: coste_camino(l, &camino), camino, expandidos });
        }
        for v in l.vecinos(p) {
            let iv = l.indice(v);
            if !visto[iv] {
                padre[iv] = ip;
                pila.push(v);
            }
        }
    }
    None
}

/// Dijkstra / A*: montículo de mínimos (`BinaryHeap` es de máximos, de ahí
/// `Reverse`) ordenado por f = g + h. En caso de empate en f se prefiere
/// mayor g (más cerca de la meta), lo que reduce los expandidos de A*.
fn mejor_primero(l: &Laberinto, h: impl Fn(Pos) -> u32) -> Option<Solucion> {
    let n = l.celdas.len();
    let mut g = vec![u32::MAX; n];
    let mut padre = vec![usize::MAX; n];
    let mut cerrado = vec![false; n];
    let mut abiertos = BinaryHeap::new();
    let i0 = l.indice(l.inicio);
    g[i0] = 0;
    abiertos.push((Reverse(h(l.inicio)), 0u32, i0));
    let mut expandidos = 0;
    while let Some((_, gp, ip)) = abiertos.pop() {
        // "Lazy deletion": puede haber entradas obsoletas de la misma
        // casilla con g peor; se ignoran.
        if cerrado[ip] {
            continue;
        }
        cerrado[ip] = true;
        expandidos += 1;
        let p = (ip / l.ancho, ip % l.ancho);
        if p == l.salida {
            return Some(Solucion { camino: reconstruir(l, &padre, p), coste: gp, expandidos });
        }
        for v in l.vecinos(p) {
            let iv = l.indice(v);
            let ng = gp + l.celda(v).coste().unwrap_or(u32::MAX);
            if ng < g[iv] {
                g[iv] = ng;
                padre[iv] = ip;
                abiertos.push((Reverse(ng + h(v)), ng, iv));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_camino_devuelve_none() {
        let l = Laberinto::parsear("S#E").unwrap();
        for a in [Algoritmo::Bfs, Algoritmo::Dfs, Algoritmo::Dijkstra, Algoritmo::AEstrella] {
            assert_eq!(resolver(&l, a), None);
        }
    }

    #[test]
    fn el_barro_desvia_a_dijkstra_pero_no_a_bfs() {
        // Recto por el barro: 3 pasos, coste 1+5+1... = 11. Rodeando: 5 pasos, coste 5.
        let l = Laberinto::parsear("S~~E\n....").unwrap();
        let b = resolver(&l, Algoritmo::Bfs).unwrap();
        let d = resolver(&l, Algoritmo::Dijkstra).unwrap();
        assert_eq!((b.camino.len() - 1, b.coste), (3, 11));
        assert_eq!((d.camino.len() - 1, d.coste), (5, 5));
    }
}
