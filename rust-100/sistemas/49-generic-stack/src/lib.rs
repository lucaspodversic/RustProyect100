//! # Stack genérico con traits
//!
//! Demuestra genéricos, traits (`StackLike`), lifetimes y un iterador
//! que presta referencias. Incluye un decorador que registra operaciones.

/// Trait de pila: push/pop/peek/len.
pub trait StackLike {
    type Item;
    fn push(&mut self, item: Self::Item);
    fn pop(&mut self) -> Option<Self::Item>;
    fn peek(&self) -> Option<&Self::Item>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }
}

/// Pila basada en `Vec`.
#[derive(Debug, Default, Clone)]
pub struct Stack<T> {
    data: Vec<T>,
}

impl<T> Stack<T> {
    pub fn new() -> Self { Self { data: Vec::new() } }
    pub fn iter(&self) -> Iter<'_, T> { Iter { inner: self.data.iter().rev() } }
}

impl<T> StackLike for Stack<T> {
    type Item = T;
    fn push(&mut self, item: T) { self.data.push(item); }
    fn pop(&mut self) -> Option<T> { self.data.pop() }
    fn peek(&self) -> Option<&T> { self.data.last() }
    fn len(&self) -> usize { self.data.len() }
}

/// Iterador de arriba hacia abajo.
pub struct Iter<'a, T> {
    inner: std::iter::Rev<std::slice::Iter<'a, T>>,
}
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> { self.inner.next() }
}

/// Decorador que cuenta operaciones sobre cualquier `StackLike`.
pub struct Traced<S: StackLike> {
    pub inner: S,
    pub pushes: usize,
    pub pops: usize,
}

impl<S: StackLike> Traced<S> {
    pub fn new(inner: S) -> Self { Self { inner, pushes: 0, pops: 0 } }
}

impl<S: StackLike> StackLike for Traced<S> {
    type Item = S::Item;
    fn push(&mut self, item: S::Item) { self.pushes += 1; self.inner.push(item); }
    fn pop(&mut self) -> Option<S::Item> { self.pops += 1; self.inner.pop() }
    fn peek(&self) -> Option<&S::Item> { self.inner.peek() }
    fn len(&self) -> usize { self.inner.len() }
}

/// Capacidad máxima: push falla (devuelve el ítem) si está llena.
pub struct Bounded<T> {
    stack: Stack<T>,
    cap: usize,
}

impl<T> Bounded<T> {
    pub fn new(cap: usize) -> Self { Self { stack: Stack::new(), cap } }
    pub fn try_push(&mut self, item: T) -> Result<(), T> {
        if self.stack.len() >= self.cap { Err(item) } else { self.stack.push(item); Ok(()) }
    }
}

impl<T> StackLike for Bounded<T> {
    type Item = T;
    fn push(&mut self, item: T) {
        let _ = self.try_push(item);
    }
    fn pop(&mut self) -> Option<T> { self.stack.pop() }
    fn peek(&self) -> Option<&T> { self.stack.peek() }
    fn len(&self) -> usize { self.stack.len() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifo_e_iter() {
        let mut s = Stack::new();
        s.push(1); s.push(2); s.push(3);
        assert_eq!(s.peek(), Some(&3));
        assert_eq!(s.iter().copied().collect::<Vec<_>>(), vec![3, 2, 1]);
        assert_eq!(s.pop(), Some(3));
    }
    #[test]
    fn traced_y_bounded() {
        let mut t = Traced::new(Stack::new());
        t.push("a"); t.push("b");
        assert_eq!(t.pop(), Some("b"));
        assert_eq!((t.pushes, t.pops), (2, 1));
        let mut b = Bounded::new(1);
        assert!(b.try_push(1).is_ok());
        assert_eq!(b.try_push(2).unwrap_err(), 2);
    }
}
