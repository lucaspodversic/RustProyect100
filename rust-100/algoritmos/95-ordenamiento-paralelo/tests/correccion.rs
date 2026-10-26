//! Todos los algoritmos deben producir exactamente lo mismo que `sort` de la std.

use ordenamiento_paralelo::datos::{generar, Distribucion};
use ordenamiento_paralelo::paralelo::{con_hilos, merge_sort_par, quicksort_par};
use ordenamiento_paralelo::secuencial::{insercion, merge_sort, quicksort};
use ordenamiento_paralelo::esta_ordenado;

type Algoritmo = fn(&mut [u64]);

const ALGORITMOS: [(&str, Algoritmo); 5] = [
    ("insercion", insercion),
    ("merge", merge_sort),
    ("quick", quicksort),
    ("merge_par", merge_sort_par),
    ("quick_par", quicksort_par),
];

#[test]
fn todas_las_distribuciones_y_tamanos() {
    let dists = [Distribucion::Aleatoria, Distribucion::Ordenada, Distribucion::Inversa, Distribucion::PocasClaves];
    for n in [0, 1, 2, 31, 33, 1000, 5000, 100_000] {
        for d in dists {
            let base = generar(n, d, 99);
            let mut esperado = base.clone();
            esperado.sort();
            for (nombre, f) in ALGORITMOS {
                if nombre == "insercion" && n > 5000 {
                    continue; // O(n²)
                }
                let mut v = base.clone();
                f(&mut v);
                assert_eq!(v, esperado, "{nombre} n={n} {d:?}");
            }
        }
    }
}

#[test]
fn merge_sort_es_estable() {
    // Pares (clave, índice original): ordenando solo por clave, los iguales
    // deben conservar el orden de entrada. Codificamos en u64: clave<<32 | idx
    // no sirve (ordenaría por índice también), así que usamos un tipo propio.
    #[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
    struct Par(u8, u32);
    impl PartialOrd for Par {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for Par {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            self.0.cmp(&o.0)
        }
    }
    let mut v: Vec<Par> = generar(50_000, Distribucion::PocasClaves, 5)
        .into_iter()
        .enumerate()
        .map(|(i, k)| Par(k as u8, i as u32))
        .collect();
    merge_sort_par(&mut v);
    assert!(esta_ordenado(&v));
    assert!(v.windows(2).all(|w| w[0].0 != w[1].0 || w[0].1 < w[1].1), "no estable");
}

#[test]
fn funciona_con_un_solo_hilo() {
    let mut v = generar(200_000, Distribucion::Aleatoria, 1);
    con_hilos(1, || quicksort_par(&mut v));
    assert!(esta_ordenado(&v));
}
