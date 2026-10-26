//! Versiones paralelas con `rayon::join`.

use crate::secuencial::{insercion, mezclar, particion3, pivote_mediana3, quicksort, UMBRAL_INSERCION};

/// Tamaño por debajo del cual no compensa crear tareas paralelas. Con
/// ~4096 elementos, ordenar cuesta decenas de microsegundos, muy por encima
/// del coste de `join` (≈ cientos de nanosegundos).
pub const UMBRAL_PARALELO: usize = 4096;

/// Merge sort paralelo. Cada mitad recibe su **propia mitad del buffer**
/// (`split_at_mut`), así que las tareas no comparten memoria mutable y el
/// compilador garantiza que no hay *data races*.
pub fn merge_sort_par<T: Ord + Copy + Default + Send>(v: &mut [T]) {
    let mut buf = vec![T::default(); v.len()];
    merge_par_rec(v, &mut buf);
}

fn merge_par_rec<T: Ord + Copy + Send>(v: &mut [T], buf: &mut [T]) {
    if v.len() <= UMBRAL_INSERCION {
        insercion(v);
        return;
    }
    let medio = v.len() / 2;
    if v.len() <= UMBRAL_PARALELO {
        // Secuencial: misma recursión pero sin join.
        let (a, b) = v.split_at_mut(medio);
        let (ba, bb) = buf.split_at_mut(medio);
        merge_par_rec(a, ba);
        merge_par_rec(b, bb);
    } else {
        let (a, b) = v.split_at_mut(medio);
        let (ba, bb) = buf.split_at_mut(medio);
        rayon::join(|| merge_par_rec(a, ba), || merge_par_rec(b, bb));
    }
    // La mezcla final es secuencial: O(n) en el nivel superior. Por eso el
    // speedup máximo de este merge sort está acotado (ley de Amdahl).
    // Una mezcla paralela (dividir por búsqueda binaria) lo mejoraría.
    mezclar(v, medio, buf);
}

/// Quicksort paralelo: tras particionar, las dos zonas se ordenan en
/// paralelo. La partición en sí es secuencial.
pub fn quicksort_par<T: Ord + Copy + Send>(v: &mut [T]) {
    if v.len() <= UMBRAL_PARALELO {
        quicksort(v);
        return;
    }
    let p = pivote_mediana3(v);
    let (lt, gt) = particion3(v, p);
    let (izq, resto) = v.split_at_mut(lt);
    let der = &mut resto[gt - lt..];
    rayon::join(|| quicksort_par(izq), || quicksort_par(der));
}

/// Ejecuta `f` en un *pool* de rayon con `hilos` hilos (para medir el
/// escalado). Con el pool global se usan todos los núcleos.
pub fn con_hilos<R: Send>(hilos: usize, f: impl FnOnce() -> R + Send) -> R {
    rayon::ThreadPoolBuilder::new()
        .num_threads(hilos)
        .build()
        .expect("crear pool")
        .install(f)
}
