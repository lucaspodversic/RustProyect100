//! Caché con TTL y reloj inyectable.
//!
//! - Clave: coordenadas **redondeadas a 2 decimales** (~1 km): consultas
//!   casi idénticas comparten entrada.
//! - Entradas vencidas no se borran enseguida: se usan como respaldo
//!   *stale-if-error* si el proveedor falla.
//! - Capacidad máxima: al llenarse se expulsa la entrada más vieja.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub trait Reloj: Send + Sync + 'static {
    fn ahora(&self) -> Instant;
}
pub struct RelojReal;
impl Reloj for RelojReal {
    fn ahora(&self) -> Instant {
        Instant::now()
    }
}

pub type Clave = (i32, i32);

pub fn clave(lat: f64, lon: f64) -> Clave {
    ((lat * 100.0).round() as i32, (lon * 100.0).round() as i32)
}

pub enum Busqueda<V> {
    Fresco(V),
    Vencido(V),
    Nada,
}

pub struct Cache<V> {
    mapa: Mutex<HashMap<Clave, (Instant, V)>>,
    ttl: Duration,
    capacidad: usize,
}

impl<V: Clone> Cache<V> {
    pub fn new(ttl: Duration, capacidad: usize) -> Self {
        Cache { mapa: Mutex::default(), ttl, capacidad }
    }

    pub fn buscar(&self, k: Clave, ahora: Instant) -> Busqueda<V> {
        match self.mapa.lock().expect("mutex").get(&k) {
            Some((t, v)) if ahora.duration_since(*t) < self.ttl => Busqueda::Fresco(v.clone()),
            Some((_, v)) => Busqueda::Vencido(v.clone()),
            None => Busqueda::Nada,
        }
    }

    pub fn guardar(&self, k: Clave, v: V, ahora: Instant) {
        let mut m = self.mapa.lock().expect("mutex");
        if m.len() >= self.capacidad && !m.contains_key(&k) {
            if let Some(vieja) = m.iter().min_by_key(|(_, (t, _))| *t).map(|(k, _)| *k) {
                m.remove(&vieja);
            }
        }
        m.insert(k, (ahora, v));
    }

    pub fn len(&self) -> usize {
        self.mapa.lock().expect("mutex").len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ttl_y_capacidad() {
        let c = Cache::new(Duration::from_secs(10), 2);
        let t0 = Instant::now();
        c.guardar(clave(1.0, 1.0), "a", t0);
        assert!(matches!(c.buscar(clave(1.001, 0.999), t0), Busqueda::Fresco("a")));
        assert!(matches!(c.buscar(clave(1.0, 1.0), t0 + Duration::from_secs(11)), Busqueda::Vencido("a")));
        c.guardar(clave(2.0, 2.0), "b", t0 + Duration::from_secs(1));
        c.guardar(clave(3.0, 3.0), "c", t0 + Duration::from_secs(2)); // expulsa "a"
        assert_eq!(c.len(), 2);
        assert!(matches!(c.buscar(clave(1.0, 1.0), t0), Busqueda::Nada));
    }
}
