//! Propiedades sobre cientos de laberintos generados al azar.

use laberintos::busqueda::{coste_camino, es_valido, resolver, Algoritmo};
use laberintos::generador::{generar, Opciones};

const TODOS: [Algoritmo; 4] = [Algoritmo::Bfs, Algoritmo::Dfs, Algoritmo::Dijkstra, Algoritmo::AEstrella];

#[test]
fn caminos_validos_y_costes_correctos() {
    for semilla in 1..150 {
        let l = generar(Opciones { columnas: 12, filas: 9, ciclos: 15, barro: 20, semilla });
        for a in TODOS {
            let s = resolver(&l, a).expect("el generador siempre conecta todo");
            assert!(es_valido(&l, &s.camino), "{a:?} semilla {semilla}");
            assert_eq!(s.coste, coste_camino(&l, &s.camino));
        }
    }
}

#[test]
fn a_estrella_optimo_y_expande_menos_que_dijkstra() {
    let (mut exp_a, mut exp_d) = (0, 0);
    for semilla in 1..150 {
        let l = generar(Opciones { columnas: 15, filas: 15, ciclos: 20, barro: 15, semilla });
        let d = resolver(&l, Algoritmo::Dijkstra).unwrap();
        let a = resolver(&l, Algoritmo::AEstrella).unwrap();
        let b = resolver(&l, Algoritmo::Bfs).unwrap();
        let p = resolver(&l, Algoritmo::Dfs).unwrap();
        assert_eq!(a.coste, d.coste, "A* debe ser óptimo (semilla {semilla})");
        assert!(d.coste <= b.coste && d.coste <= p.coste);
        assert!(b.camino.len() <= p.camino.len(), "BFS minimiza pasos");
        exp_a += a.expandidos;
        exp_d += d.expandidos;
    }
    assert!(exp_a < exp_d, "A* {exp_a} vs Dijkstra {exp_d}");
}

#[test]
fn laberinto_perfecto_todos_coinciden() {
    // Sin ciclos hay un único camino: los cuatro algoritmos dan el mismo.
    for semilla in 1..50 {
        let l = generar(Opciones { columnas: 10, filas: 10, ciclos: 0, barro: 0, semilla });
        let ref_ = resolver(&l, Algoritmo::Bfs).unwrap().camino;
        for a in TODOS {
            assert_eq!(resolver(&l, a).unwrap().camino, ref_);
        }
    }
}
