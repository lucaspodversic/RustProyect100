//! Árbol binario de búsqueda (BST) que funciona como mapa ordenado `K → V`.
//!
//! Invariante: para cada nodo, todas las claves del subárbol izquierdo son
//! menores y todas las del derecho, mayores.
//!
//! ```text
//!         8
//!       /   \
//!      3     10
//!     / \      \
//!    1   6      14
//! ```
//!
//! El costo de las operaciones es O(h), donde h es la altura. Con claves
//! aleatorias h ≈ 2·ln n, pero con claves **ordenadas** el árbol degenera en
//! una lista (h = n). Los árboles AVL o rojo‑negro (como `BTreeMap`, que en
//! realidad es un árbol B) evitan esto rebalanceando. Aquí se deja sin
//! balancear a propósito para que se vea el problema (ver `altura`).
//!
//! Las operaciones son **iterativas** (con cursores `&mut Enlace`) en vez de
//! recursivas, para no desbordar la pila con árboles degenerados.

use core::cmp::Ordering;

type Enlace<K, V> = Option<Box<Nodo<K, V>>>;

struct Nodo<K, V> {
    clave: K,
    valor: V,
    izq: Enlace<K, V>,
    der: Enlace<K, V>,
}

/// Árbol binario de búsqueda.
pub struct Arbol<K, V> {
    raiz: Enlace<K, V>,
    largo: usize,
}

impl<K: Ord, V> Default for Arbol<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V> Arbol<K, V> {
    /// Árbol vacío.
    pub const fn new() -> Self {
        Self { raiz: None, largo: 0 }
    }

    /// Número de claves.
    pub fn len(&self) -> usize {
        self.largo
    }

    /// ¿Vacío?
    pub fn is_empty(&self) -> bool {
        self.largo == 0
    }

    /// Inserta o reemplaza. Devuelve el valor anterior si la clave existía.
    pub fn insertar(&mut self, clave: K, valor: V) -> Option<V> {
        let mut cursor = &mut self.raiz;
        // Se baja por el árbol hasta un hueco (None) o hasta la clave.
        while let Some(nodo) = cursor {
            match clave.cmp(&nodo.clave) {
                Ordering::Less => cursor = &mut nodo.izq,
                Ordering::Greater => cursor = &mut nodo.der,
                Ordering::Equal => return Some(core::mem::replace(&mut nodo.valor, valor)),
            }
        }
        *cursor = Some(Box::new(Nodo { clave, valor, izq: None, der: None }));
        self.largo += 1;
        None
    }

    /// Busca una clave.
    pub fn obtener(&self, clave: &K) -> Option<&V> {
        let mut actual = self.raiz.as_deref();
        while let Some(n) = actual {
            actual = match clave.cmp(&n.clave) {
                Ordering::Less => n.izq.as_deref(),
                Ordering::Greater => n.der.as_deref(),
                Ordering::Equal => return Some(&n.valor),
            };
        }
        None
    }

    /// ¿Contiene la clave?
    pub fn contiene(&self, clave: &K) -> bool {
        self.obtener(clave).is_some()
    }

    /// Elimina una clave. Hay tres casos:
    /// 1. **Hoja**: se quita sin más.
    /// 2. **Un hijo**: el hijo ocupa su lugar.
    /// 3. **Dos hijos**: se reemplaza por su **sucesor** (el mínimo del
    ///    subárbol derecho), que tiene como mucho un hijo, y se elimina el
    ///    sucesor de su posición original.
    pub fn eliminar(&mut self, clave: &K) -> Option<V> {
        // 1) Localizar el enlace que apunta al nodo.
        let mut cursor = &mut self.raiz;
        loop {
            match cursor {
                None => return None,
                Some(n) if *clave == n.clave => break,
                Some(n) => {
                    cursor = if *clave < n.clave { &mut n.izq } else { &mut n.der };
                }
            }
        }
        let mut nodo = cursor.take().expect("se encontró arriba");
        self.largo -= 1;
        *cursor = match (nodo.izq.take(), nodo.der.take()) {
            (None, None) => None,
            (Some(h), None) | (None, Some(h)) => Some(h),
            (Some(izq), Some(der)) => {
                let (mut sucesor, resto_der) = Self::extraer_minimo(der);
                sucesor.izq = Some(izq);
                sucesor.der = resto_der;
                Some(sucesor)
            }
        };
        Some(nodo.valor)
    }

    /// Separa el nodo mínimo de un subárbol. Devuelve (mínimo, subárbol restante).
    fn extraer_minimo(mut raiz: Box<Nodo<K, V>>) -> (Box<Nodo<K, V>>, Enlace<K, V>) {
        if raiz.izq.is_none() {
            let resto = raiz.der.take();
            return (raiz, resto);
        }
        // Bajar por la izquierda hasta el padre del mínimo.
        let mut cursor = &mut raiz.izq;
        while cursor.as_ref().is_some_and(|n| n.izq.is_some()) {
            cursor = &mut cursor.as_mut().expect("comprobado").izq;
        }
        let mut minimo = cursor.take().expect("existe");
        *cursor = minimo.der.take();
        (minimo, Some(raiz))
    }

    /// Clave mínima.
    pub fn minimo(&self) -> Option<&K> {
        let mut n = self.raiz.as_deref()?;
        while let Some(i) = n.izq.as_deref() {
            n = i;
        }
        Some(&n.clave)
    }

    /// Clave máxima.
    pub fn maximo(&self) -> Option<&K> {
        let mut n = self.raiz.as_deref()?;
        while let Some(d) = n.der.as_deref() {
            n = d;
        }
        Some(&n.clave)
    }

    /// Altura (0 = vacío). Recorrido por niveles con una pila explícita.
    pub fn altura(&self) -> usize {
        let mut max = 0;
        let mut pila: Vec<(&Nodo<K, V>, usize)> = self.raiz.as_deref().map(|r| (r, 1)).into_iter().collect();
        while let Some((n, h)) = pila.pop() {
            max = max.max(h);
            for hijo in [n.izq.as_deref(), n.der.as_deref()].into_iter().flatten() {
                pila.push((hijo, h + 1));
            }
        }
        max
    }

    /// Iterador **en orden** (claves ascendentes). Usa una pila con el
    /// "camino izquierdo" pendiente: es el recorrido in‑order iterativo.
    pub fn iter(&self) -> Iter<'_, K, V> {
        let mut it = Iter { pila: Vec::new() };
        it.apilar_izquierda(self.raiz.as_deref());
        it
    }
}

impl<K, V> Drop for Arbol<K, V> {
    /// Liberación iterativa (un árbol degenerado de 10⁶ nodos desbordaría la
    /// pila con el Drop recursivo por defecto).
    fn drop(&mut self) {
        let mut pila: Vec<Box<Nodo<K, V>>> = self.raiz.take().into_iter().collect();
        while let Some(mut n) = pila.pop() {
            pila.extend(n.izq.take());
            pila.extend(n.der.take());
        }
    }
}

/// Iterador en orden.
pub struct Iter<'a, K, V> {
    pila: Vec<&'a Nodo<K, V>>,
}

impl<'a, K, V> Iter<'a, K, V> {
    fn apilar_izquierda(&mut self, mut n: Option<&'a Nodo<K, V>>) {
        while let Some(x) = n {
            self.pila.push(x);
            n = x.izq.as_deref();
        }
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        let n = self.pila.pop()?;
        self.apilar_izquierda(n.der.as_deref());
        Some((&n.clave, &n.valor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ejemplo() -> Arbol<i32, &'static str> {
        let mut a = Arbol::new();
        for k in [8, 3, 10, 1, 6, 14, 4, 7, 13] {
            a.insertar(k, "x");
        }
        a
    }

    #[test]
    fn insertar_buscar_y_orden() {
        let mut a = ejemplo();
        assert_eq!(a.len(), 9);
        assert_eq!(a.insertar(6, "seis"), Some("x"));
        assert_eq!(a.obtener(&6), Some(&"seis"));
        assert!(!a.contiene(&5));
        let claves: Vec<_> = a.iter().map(|(k, _)| *k).collect();
        assert_eq!(claves, vec![1, 3, 4, 6, 7, 8, 10, 13, 14]);
        assert_eq!((a.minimo(), a.maximo()), (Some(&1), Some(&14)));
        assert_eq!(a.altura(), 4);
    }

    #[test]
    fn eliminar_los_tres_casos() {
        let mut a = ejemplo();
        assert_eq!(a.eliminar(&13), Some("x")); // hoja
        assert_eq!(a.eliminar(&10), Some("x")); // un hijo
        assert_eq!(a.eliminar(&3), Some("x")); // dos hijos (sucesor 4)
        assert_eq!(a.eliminar(&8), Some("x")); // la raíz con dos hijos
        assert_eq!(a.eliminar(&99), None);
        let claves: Vec<_> = a.iter().map(|(k, _)| *k).collect();
        assert_eq!(claves, vec![1, 4, 6, 7, 14]);
        assert_eq!(a.len(), 5);
    }

    #[test]
    fn claves_ordenadas_degeneran() {
        let mut a = Arbol::new();
        for i in 0..20_000 {
            a.insertar(i, ());
        }
        assert_eq!(a.altura(), 20_000); // ¡una lista!
        drop(a); // no desborda la pila
    }
}
