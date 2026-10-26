//! Algoritmos secuenciales de referencia.

/// Por debajo de este tamaño se usa inserción.
pub const UMBRAL_INSERCION: usize = 32;

/// Ordenamiento por inserción: O(n²), pero muy rápido para n pequeño
/// (poco *overhead* y accesos contiguos a memoria).
pub fn insercion<T: Ord>(v: &mut [T]) {
    // Invariante: v[..i] ya está ordenado; "hundimos" v[i] hasta su sitio.
    for i in 1..v.len() {
        let mut j = i;
        while j > 0 && v[j - 1] > v[j] {
            v.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// Mezcla dos mitades ordenadas `v[..medio]` y `v[medio..]` usando `buf`
/// (de longitud ≥ medio) como espacio auxiliar. Se copia solo la mitad
/// izquierda al buffer, y la mezcla escribe sobre `v` desde el principio.
/// Así nunca se pisa un dato que aún no se leyó.
pub fn mezclar<T: Ord + Copy>(v: &mut [T], medio: usize, buf: &mut [T]) {
    // Atajo: si ya están en orden no hay nada que mezclar (muy útil con
    // entradas casi ordenadas).
    if medio == 0 || medio == v.len() || v[medio - 1] <= v[medio] {
        return;
    }
    buf[..medio].copy_from_slice(&v[..medio]);
    let (mut i, mut j, mut k) = (0, medio, 0);
    while i < medio && j < v.len() {
        // `<=` hace que el algoritmo sea ESTABLE (los iguales mantienen el orden).
        if buf[i] <= v[j] {
            v[k] = buf[i];
            i += 1;
        } else {
            v[k] = v[j];
            j += 1;
        }
        k += 1;
    }
    // Lo que quede de la derecha ya está en su sitio; solo falta el resto
    // de la izquierda.
    v[k..k + (medio - i)].copy_from_slice(&buf[i..medio]);
}

/// Merge sort *top‑down* con buffer auxiliar reutilizado. O(n log n), estable.
pub fn merge_sort<T: Ord + Copy + Default>(v: &mut [T]) {
    let mut buf = vec![T::default(); v.len() / 2 + 1];
    merge_rec(v, &mut buf);
}

fn merge_rec<T: Ord + Copy>(v: &mut [T], buf: &mut [T]) {
    if v.len() <= UMBRAL_INSERCION {
        insercion(v);
        return;
    }
    let medio = v.len() / 2;
    merge_rec(&mut v[..medio], buf);
    merge_rec(&mut v[medio..], buf);
    mezclar(v, medio, buf);
}

/// Elige el pivote con la **mediana de tres posiciones pseudoaleatorias**.
///
/// La mediana de (primero, medio, último) clásica evita el peor caso con
/// entradas ordenadas, PERO la partición en tres vías desordena la parte
/// derecha formando patrones tipo "órgano" (subida‑bajada) contra los que
/// esa elección vuelve a ser O(n²). Lo comprobamos en el benchmark: 1,1 s
/// para 2 M ya ordenados. Con posiciones pseudoaleatorias (derivadas de la
/// longitud con xorshift, así el resultado es determinista) ningún patrón
/// fijo de entrada provoca el peor caso de forma sistemática.
pub fn pivote_mediana3<T: Ord + Copy>(v: &[T]) -> T {
    let n = v.len() as u64;
    let mut x = n.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut idx = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x % n) as usize
    };
    let (a, b, c) = (v[idx()], v[idx()], v[idx()]);
    // Mediana de tres sin ordenar: el que queda "entre" los otros dos.
    if (a <= b) == (b <= c) {
        b
    } else if (b <= a) == (a <= c) {
        a
    } else {
        c
    }
}

/// Partición en **tres vías** (bandera holandesa de Dijkstra): deja
/// `[< p][== p][> p]` y devuelve los límites `(lt, gt)`. Con muchos
/// duplicados, la zona central no se vuelve a procesar.
pub fn particion3<T: Ord + Copy>(v: &mut [T], p: T) -> (usize, usize) {
    let (mut lt, mut i, mut gt) = (0, 0, v.len());
    while i < gt {
        if v[i] < p {
            v.swap(lt, i);
            lt += 1;
            i += 1;
        } else if v[i] > p {
            gt -= 1;
            v.swap(i, gt);
        } else {
            i += 1;
        }
    }
    (lt, gt)
}

/// Quicksort con mediana de tres y partición en tres vías. Recursa sobre la
/// parte **menor** y continúa en bucle con la mayor, así la profundidad de
/// la pila queda acotada a O(log n).
pub fn quicksort<T: Ord + Copy>(mut v: &mut [T]) {
    loop {
        if v.len() <= UMBRAL_INSERCION {
            insercion(v);
            return;
        }
        let p = pivote_mediana3(v);
        let (lt, gt) = particion3(v, p);
        let (izq, resto) = v.split_at_mut(lt);
        let der = &mut resto[gt - lt..];
        if izq.len() < der.len() {
            quicksort(izq);
            v = der;
        } else {
            quicksort(der);
            v = izq;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particion_tres_vias_deja_tres_zonas() {
        let mut v = [5, 1, 5, 9, 2, 5, 7];
        let (lt, gt) = particion3(&mut v, 5);
        assert!(v[..lt].iter().all(|&x| x < 5));
        assert!(v[lt..gt].iter().all(|&x| x == 5));
        assert!(v[gt..].iter().all(|&x| x > 5));
        assert_eq!(gt - lt, 3);
    }

    #[test]
    fn mezcla_estable_y_atajo() {
        let mut v = [1, 4, 7, 2, 3, 9];
        let mut buf = [0; 3];
        mezclar(&mut v, 3, &mut buf);
        assert_eq!(v, [1, 2, 3, 4, 7, 9]);
        let mut ya = [1, 2, 3, 4];
        mezclar(&mut ya, 2, &mut buf); // atajo: no toca nada
        assert_eq!(ya, [1, 2, 3, 4]);
    }

    #[test]
    fn pivote_es_un_elemento_del_slice() {
        let v: Vec<u32> = (0..100).rev().collect();
        assert!(v.contains(&pivote_mediana3(&v)));
    }
}
