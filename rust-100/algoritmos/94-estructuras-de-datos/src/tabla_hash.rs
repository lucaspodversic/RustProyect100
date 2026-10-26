//! Tabla hash con **encadenamiento separado** y función hash **FNV‑1a**.
//!
//! ```text
//! cubetas: [0] → (k1,v1) → (k7,v7)
//!          [1] → ∅
//!          [2] → (k3,v3)
//!          ...
//! ```
//!
//! - **Índice**: `hash(clave) & (capacidad − 1)`. La capacidad es potencia de
//!   2, así el módulo se calcula con una máscara.
//! - **Factor de carga** = elementos / cubetas. Al superar **0,75** se duplica
//!   la capacidad y se reubican todos los elementos (*rehash*). Así las
//!   cadenas se mantienen cortas y el costo amortizado sigue siendo O(1).
//! - **FNV‑1a** (Fowler–Noll–Vo) es rápida y simple, pero **no** resiste
//!   ataques de colisiones (HashDoS). Por eso `std::HashMap` usa SipHash con
//!   una semilla aleatoria. Para usos didácticos o claves de confianza, FNV
//!   es suficiente.
//! - Cada cubeta es un `Vec<(K, V)>` (más amigable con la caché que una lista
//!   enlazada). Para eliminar se usa `swap_remove` en O(1), porque el orden
//!   dentro de la cubeta no importa.

use core::hash::{Hash, Hasher};

/// Hasher FNV‑1a de 64 bits.
#[derive(Debug, Clone, Copy)]
pub struct Fnv1a(u64);

impl Default for Fnv1a {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325) // "offset basis"
    }
}

impl Hasher for Fnv1a {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            // FNV‑1a: primero XOR y luego multiplicar (FNV‑1 lo hace al revés).
            self.0 ^= *b as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3); // primo FNV
        }
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

/// Calcula el hash FNV‑1a de un valor.
pub fn fnv1a<T: Hash + ?Sized>(v: &T) -> u64 {
    let mut h = Fnv1a::default();
    v.hash(&mut h);
    h.finish()
}

const CAPACIDAD_INICIAL: usize = 8;

/// Tabla hash.
pub struct TablaHash<K, V> {
    cubetas: Vec<Vec<(K, V)>>,
    largo: usize,
}

impl<K: Hash + Eq, V> Default for TablaHash<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Hash + Eq, V> TablaHash<K, V> {
    /// Tabla vacía (no reserva memoria hasta la primera inserción).
    pub fn new() -> Self {
        Self { cubetas: Vec::new(), largo: 0 }
    }

    /// Elementos almacenados.
    pub fn len(&self) -> usize {
        self.largo
    }

    /// ¿Vacía?
    pub fn is_empty(&self) -> bool {
        self.largo == 0
    }

    /// Número de cubetas.
    pub fn capacidad(&self) -> usize {
        self.cubetas.len()
    }

    /// Longitud de la cadena más larga (calidad del hash).
    pub fn cadena_maxima(&self) -> usize {
        self.cubetas.iter().map(Vec::len).max().unwrap_or(0)
    }

    fn indice(&self, k: &K) -> usize {
        (fnv1a(k) as usize) & (self.cubetas.len() - 1)
    }

    /// Duplica la capacidad y redistribuye.
    fn crecer(&mut self) {
        let nueva = (self.cubetas.len() * 2).max(CAPACIDAD_INICIAL);
        let viejas = core::mem::replace(&mut self.cubetas, (0..nueva).map(|_| Vec::new()).collect());
        for (k, v) in viejas.into_iter().flatten() {
            let i = self.indice(&k);
            self.cubetas[i].push((k, v));
        }
    }

    /// Inserta o reemplaza; devuelve el valor anterior.
    pub fn insertar(&mut self, k: K, v: V) -> Option<V> {
        // Crecer si con este elemento se superaría 0,75 (= 3/4, en enteros).
        if self.cubetas.is_empty() || (self.largo + 1) * 4 > self.cubetas.len() * 3 {
            self.crecer();
        }
        let i = self.indice(&k);
        let cubeta = &mut self.cubetas[i];
        if let Some(par) = cubeta.iter_mut().find(|(kk, _)| *kk == k) {
            return Some(core::mem::replace(&mut par.1, v));
        }
        cubeta.push((k, v));
        self.largo += 1;
        None
    }

    /// Obtiene un valor.
    pub fn obtener(&self, k: &K) -> Option<&V> {
        if self.cubetas.is_empty() {
            return None;
        }
        self.cubetas[self.indice(k)].iter().find(|(kk, _)| kk == k).map(|(_, v)| v)
    }

    /// Obtiene un valor mutable.
    pub fn obtener_mut(&mut self, k: &K) -> Option<&mut V> {
        if self.cubetas.is_empty() {
            return None;
        }
        let i = self.indice(k);
        self.cubetas[i].iter_mut().find(|(kk, _)| kk == k).map(|(_, v)| v)
    }

    /// Devuelve el valor de `k`, insertando `defecto()` si no existe
    /// (equivalente simplificado a `entry().or_insert_with()`).
    pub fn obtener_o_insertar(&mut self, k: K, defecto: impl FnOnce() -> V) -> &mut V {
        if self.obtener(&k).is_none() {
            // Insertar primero: puede provocar un rehash, por eso el índice
            // se calcula DESPUÉS.
            if self.cubetas.is_empty() || (self.largo + 1) * 4 > self.cubetas.len() * 3 {
                self.crecer();
            }
            let i = self.indice(&k);
            self.cubetas[i].push((k, defecto()));
            self.largo += 1;
            return &mut self.cubetas[i].last_mut().expect("recién insertado").1;
        }
        let i = self.indice(&k);
        let cubeta = &mut self.cubetas[i];
        let pos = cubeta.iter().position(|(kk, _)| *kk == k).expect("existe");
        &mut cubeta[pos].1
    }

    /// Elimina una clave.
    pub fn eliminar(&mut self, k: &K) -> Option<V> {
        if self.cubetas.is_empty() {
            return None;
        }
        let i = self.indice(k);
        let cubeta = &mut self.cubetas[i];
        let pos = cubeta.iter().position(|(kk, _)| kk == k)?;
        self.largo -= 1;
        Some(cubeta.swap_remove(pos).1)
    }

    /// Iterador sobre (clave, valor) en orden arbitrario.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.cubetas.iter().flatten().map(|(k, v)| (k, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectores_oficiales_fnv1a() {
        // Vectores de prueba de la especificación (sobre bytes crudos).
        let h = |b: &[u8]| {
            let mut f = Fnv1a::default();
            f.write(b);
            f.finish()
        };
        assert_eq!(h(b""), 0xcbf29ce484222325);
        assert_eq!(h(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(h(b"foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn crud_y_crecimiento() {
        let mut t: TablaHash<i32, i32> = TablaHash::new();
        assert_eq!(t.capacidad(), 0);
        assert_eq!(t.obtener(&1000), None);
        for i in 0..100 {
            assert_eq!(t.insertar(i, i * i), None);
        }
        assert_eq!(t.len(), 100);
        // 100 elementos con carga ≤ 0,75 → al menos 134 cubetas → 256.
        assert_eq!(t.capacidad(), 256);
        assert_eq!(t.insertar(7, 0), Some(49));
        *t.obtener_mut(&8).unwrap() += 1;
        assert_eq!(t.obtener(&8), Some(&65));
        assert_eq!(t.eliminar(&7), Some(0));
        assert_eq!(t.eliminar(&7), None);
        assert_eq!(t.len(), 99);
        assert!(t.cadena_maxima() <= 6, "cadena {}", t.cadena_maxima());
    }

    #[test]
    fn contar_palabras() {
        let mut t: TablaHash<&str, u32> = TablaHash::new();
        for p in "el gato y el perro y el pez".split_whitespace() {
            *t.obtener_o_insertar(p, || 0) += 1;
        }
        assert_eq!(t.obtener(&"el"), Some(&3));
        assert_eq!(t.obtener(&"y"), Some(&2));
        assert_eq!(t.iter().map(|(_, v)| v).sum::<u32>(), 8);
    }
}
