//! Lista simplemente enlazada.
//!
//! ```text
//! cabeza ─► [3|•]─► [2|•]─► [1|∅]
//! ```
//!
//! Cada nodo es dueño (`Box`) del siguiente: `Option<Box<Nodo<T>>>`. Gracias
//! a la *niche optimization*, `Option<Box<_>>` ocupa lo mismo que un puntero
//! (`None` se representa como el puntero nulo).
//!
//! ## Detalles importantes
//! - **`Drop` iterativo**: el `Drop` automático sería recursivo (cada `Box`
//!   libera al siguiente) y con millones de nodos desbordaría la pila. Por
//!   eso se implementa a mano con un bucle.
//! - **`Option::take`** es la herramienta clave: saca el valor y deja `None`,
//!   y así se puede mover un nodo sin dejar la lista en un estado inválido.

/// Enlace a un nodo.
type Enlace<T> = Option<Box<Nodo<T>>>;

struct Nodo<T> {
    valor: T,
    siguiente: Enlace<T>,
}

/// Lista enlazada (funciona como pila: se inserta y extrae por delante).
pub struct Lista<T> {
    cabeza: Enlace<T>,
    largo: usize,
}

impl<T> Default for Lista<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Lista<T> {
    /// Lista vacía.
    pub const fn new() -> Self {
        Self { cabeza: None, largo: 0 }
    }

    /// Número de elementos (se mantiene un contador para que sea O(1)).
    pub fn len(&self) -> usize {
        self.largo
    }

    /// ¿Está vacía?
    pub fn is_empty(&self) -> bool {
        self.cabeza.is_none()
    }

    /// Inserta al principio: O(1).
    pub fn push(&mut self, valor: T) {
        // `take` deja `self.cabeza` en None y nos da la antigua cabeza, que
        // pasa a ser el siguiente del nuevo nodo.
        let nuevo = Box::new(Nodo { valor, siguiente: self.cabeza.take() });
        self.cabeza = Some(nuevo);
        self.largo += 1;
    }

    /// Extrae el primero: O(1).
    pub fn pop(&mut self) -> Option<T> {
        self.cabeza.take().map(|nodo| {
            self.cabeza = nodo.siguiente;
            self.largo -= 1;
            nodo.valor
        })
    }

    /// Referencia al primero.
    pub fn peek(&self) -> Option<&T> {
        self.cabeza.as_ref().map(|n| &n.valor)
    }

    /// Referencia mutable al primero.
    pub fn peek_mut(&mut self) -> Option<&mut T> {
        self.cabeza.as_mut().map(|n| &mut n.valor)
    }

    /// Invierte la lista **en el sitio** en O(n) sin reservar memoria: se van
    /// "dando la vuelta" los punteros uno a uno.
    pub fn invertir(&mut self) {
        let mut previo: Enlace<T> = None;
        let mut actual = self.cabeza.take();
        while let Some(mut nodo) = actual {
            actual = nodo.siguiente.take(); // avanzar
            nodo.siguiente = previo; // apuntar hacia atrás
            previo = Some(nodo);
        }
        self.cabeza = previo;
    }

    /// Iterador por referencia.
    pub fn iter(&self) -> Iter<'_, T> {
        Iter { siguiente: self.cabeza.as_deref() }
    }

    /// Iterador por referencia mutable.
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut { siguiente: self.cabeza.as_deref_mut() }
    }

    /// ¿Contiene el valor? O(n).
    pub fn contiene(&self, v: &T) -> bool
    where
        T: PartialEq,
    {
        self.iter().any(|x| x == v)
    }

    /// Elimina la primera aparición de `v`. Recorre con un `&mut Enlace`
    /// ("cursor") para poder reenganchar el anterior con el siguiente.
    pub fn eliminar(&mut self, v: &T) -> bool
    where
        T: PartialEq,
    {
        let mut cursor = &mut self.cabeza;
        loop {
            match cursor {
                None => return false,
                Some(nodo) if nodo.valor == *v => {
                    *cursor = nodo.siguiente.take();
                    self.largo -= 1;
                    return true;
                }
                Some(nodo) => cursor = &mut nodo.siguiente,
            }
        }
    }
}

impl<T> Drop for Lista<T> {
    fn drop(&mut self) {
        // Liberación iterativa: cada nodo se suelta con su `siguiente` ya
        // vaciado, así el Drop de Box no recurre.
        let mut actual = self.cabeza.take();
        while let Some(mut nodo) = actual {
            actual = nodo.siguiente.take();
        }
    }
}

/// Iterador por referencia.
pub struct Iter<'a, T> {
    siguiente: Option<&'a Nodo<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        self.siguiente.map(|n| {
            self.siguiente = n.siguiente.as_deref();
            &n.valor
        })
    }
}

/// Iterador mutable. Usa `take()` sobre el `Option<&mut>` para no tener dos
/// referencias mutables vivas al mismo nodo.
pub struct IterMut<'a, T> {
    siguiente: Option<&'a mut Nodo<T>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<&'a mut T> {
        self.siguiente.take().map(|n| {
            self.siguiente = n.siguiente.as_deref_mut();
            &mut n.valor
        })
    }
}

/// Iterador por valor (consume la lista).
pub struct IntoIter<T>(Lista<T>);

impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.0.pop()
    }
}

impl<T> IntoIterator for Lista<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> IntoIter<T> {
        IntoIter(self)
    }
}

impl<T> FromIterator<T> for Lista<T> {
    /// Construye la lista **conservando el orden** del iterador: se inserta
    /// todo por delante y al final se invierte (O(n) total).
    fn from_iter<I: IntoIterator<Item = T>>(it: I) -> Self {
        let mut l = Lista::new();
        for v in it {
            l.push(v);
        }
        l.invertir();
        l
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for Lista<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pila_basica() {
        let mut l = Lista::new();
        assert_eq!(l.pop(), None);
        l.push(1);
        l.push(2);
        l.push(3);
        assert_eq!(l.len(), 3);
        assert_eq!(l.peek(), Some(&3));
        *l.peek_mut().unwrap() = 30;
        assert_eq!(l.pop(), Some(30));
        assert_eq!(l.pop(), Some(2));
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn invertir_iterar_y_eliminar() {
        let mut l: Lista<i32> = (1..=5).collect();
        assert_eq!(format!("{l:?}"), "[1, 2, 3, 4, 5]");
        l.invertir();
        assert_eq!(l.iter().copied().collect::<Vec<_>>(), vec![5, 4, 3, 2, 1]);
        for v in l.iter_mut() {
            *v *= 10;
        }
        assert!(l.eliminar(&30) && !l.eliminar(&99));
        assert!(l.eliminar(&50)); // la cabeza
        assert!(l.eliminar(&10)); // el último
        assert_eq!(l.into_iter().collect::<Vec<_>>(), vec![40, 20]);
    }

    #[test]
    fn drop_de_lista_enorme_no_desborda_la_pila() {
        let mut l = Lista::new();
        for i in 0..1_000_000 {
            l.push(i);
        }
        drop(l);
    }
}
