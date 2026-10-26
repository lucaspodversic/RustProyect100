//! Generación de laberintos aleatorios.
//!
//! **Backtracker recursivo** (implementado con pila explícita para no
//! desbordar con laberintos grandes): se parte de una rejilla llena de muros
//! en la que las "habitaciones" son las celdas de coordenadas impares. Desde
//! la habitación actual se elige al azar un vecino a distancia 2 sin visitar,
//! se derriba el muro intermedio y se avanza. Si no hay vecinos libres, se
//! retrocede. El resultado es un laberinto **perfecto**: un árbol, con
//! exactamente un camino entre cada par de celdas. En él BFS, DFS y A*
//! encuentran el MISMO camino.
//!
//! Por eso existe la opción `ciclos`: derriba muros extra al azar, creando
//! caminos alternativos, y entonces DFS deja de ser óptimo.

use crate::rejilla::{Celda, Laberinto};

/// PRNG xorshift64 (determinista con la misma semilla).
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// Nuevo generador (la semilla 0 se corrige, xorshift no la admite).
    pub fn new(semilla: u64) -> Self {
        Rng(semilla.max(1))
    }
    /// Siguiente número.
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Entero en `0..n`.
    pub fn rango(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Parámetros del generador.
#[derive(Debug, Clone, Copy)]
pub struct Opciones {
    /// Habitaciones por fila (el ancho real es 2·n+1).
    pub columnas: usize,
    /// Habitaciones por columna.
    pub filas: usize,
    /// Porcentaje (0‑100) de muros interiores extra que se derriban.
    pub ciclos: u8,
    /// Porcentaje (0‑100) de casillas libres convertidas en barro.
    pub barro: u8,
    /// Semilla.
    pub semilla: u64,
}

/// Genera un laberinto. El inicio está arriba a la izquierda y la salida abajo a la derecha.
pub fn generar(op: Opciones) -> Laberinto {
    assert!(op.filas > 0 && op.columnas > 0, "dimensiones vacías");
    let (alto, ancho) = (2 * op.filas + 1, 2 * op.columnas + 1);
    let mut l = Laberinto {
        ancho,
        alto,
        celdas: vec![Celda::Muro; ancho * alto],
        inicio: (1, 1),
        salida: (alto - 2, ancho - 2),
    };
    let mut rng = Rng::new(op.semilla);
    let mut visitado = vec![false; op.filas * op.columnas];
    // La pila guarda habitaciones (fila, col) en coordenadas de habitación.
    let mut pila = vec![(0usize, 0usize)];
    visitado[0] = true;
    l.poner((1, 1), Celda::Libre);
    while let Some(&(f, c)) = pila.last() {
        // Candidatos: vecinos a distancia 1 en coordenadas de habitación.
        let mut cand = [(0, 0); 4];
        let mut n = 0;
        for (df, dc) in [(-1i32, 0i32), (0, 1), (1, 0), (0, -1)] {
            let (nf, nc) = (f as i32 + df, c as i32 + dc);
            if nf >= 0 && nc >= 0 && (nf as usize) < op.filas && (nc as usize) < op.columnas {
                let (nf, nc) = (nf as usize, nc as usize);
                if !visitado[nf * op.columnas + nc] {
                    cand[n] = (nf, nc);
                    n += 1;
                }
            }
        }
        if n == 0 {
            pila.pop(); // callejón sin salida: retroceder
            continue;
        }
        let (nf, nc) = cand[rng.rango(n)];
        visitado[nf * op.columnas + nc] = true;
        // Derribar el muro intermedio y abrir la habitación nueva.
        l.poner((f + nf + 1, c + nc + 1), Celda::Libre);
        l.poner((2 * nf + 1, 2 * nc + 1), Celda::Libre);
        pila.push((nf, nc));
    }
    // Ciclos: muros interiores que separan dos casillas libres (en línea).
    if op.ciclos > 0 {
        for f in 1..alto - 1 {
            for c in 1..ancho - 1 {
                if l.celda((f, c)) != Celda::Muro {
                    continue;
                }
                let horizontal = l.celda((f, c - 1)) != Celda::Muro && l.celda((f, c + 1)) != Celda::Muro;
                let vertical = l.celda((f - 1, c)) != Celda::Muro && l.celda((f + 1, c)) != Celda::Muro;
                if (horizontal ^ vertical) && rng.rango(100) < op.ciclos as usize {
                    l.poner((f, c), Celda::Libre);
                }
            }
        }
    }
    if op.barro > 0 {
        for i in 0..l.celdas.len() {
            if l.celdas[i] == Celda::Libre && rng.rango(100) < op.barro as usize {
                l.celdas[i] = Celda::Barro;
            }
        }
        // El inicio y la salida siempre libres.
        let (s, e) = (l.inicio, l.salida);
        l.poner(s, Celda::Libre);
        l.poner(e, Celda::Libre);
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laberinto_perfecto_es_un_arbol() {
        // En un árbol: aristas = nodos − 1. Contamos las casillas libres
        // (nodos) y las adyacencias entre libres (aristas).
        let l = generar(Opciones { columnas: 15, filas: 10, ciclos: 0, barro: 0, semilla: 42 });
        let libres: Vec<_> = (0..l.alto)
            .flat_map(|f| (0..l.ancho).map(move |c| (f, c)))
            .filter(|&p| l.celda(p) != Celda::Muro)
            .collect();
        let aristas: usize = libres.iter().map(|&p| l.vecinos(p).count()).sum::<usize>() / 2;
        assert_eq!(aristas, libres.len() - 1);
    }

    #[test]
    fn determinista_con_semilla() {
        let op = Opciones { columnas: 8, filas: 8, ciclos: 10, barro: 10, semilla: 7 };
        assert_eq!(generar(op), generar(op));
    }
}
