//! Pruebas "diferenciales": se aplican las mismas operaciones aleatorias a
//! nuestra estructura y a su equivalente de la std, y se comprueba que
//! siempre coinciden. Es una forma barata de *property testing*.

use estructuras_de_datos::xorshift::XorShift;
use estructuras_de_datos::{Arbol, Lista, TablaHash};
use std::collections::{BTreeMap, HashMap};

#[test]
fn arbol_vs_btreemap() {
    let mut rng = XorShift::new(42);
    let mut a = Arbol::new();
    let mut b = BTreeMap::new();
    for _ in 0..20_000 {
        let k = rng.rango(500);
        match rng.rango(3) {
            0 | 1 => assert_eq!(a.insertar(k, k * 2), b.insert(k, k * 2)),
            _ => assert_eq!(a.eliminar(&k), b.remove(&k)),
        }
        assert_eq!(a.len(), b.len());
    }
    assert!(a.iter().map(|(k, v)| (*k, *v)).eq(b.iter().map(|(k, v)| (*k, *v))));
    assert_eq!(a.minimo(), b.keys().next());
    assert_eq!(a.maximo(), b.keys().next_back());
}

#[test]
fn tabla_vs_hashmap() {
    let mut rng = XorShift::new(7);
    let mut t = TablaHash::new();
    let mut h = HashMap::new();
    for _ in 0..50_000 {
        let k = format!("clave{}", rng.rango(2000));
        match rng.rango(4) {
            0 | 1 => assert_eq!(t.insertar(k.clone(), 1), h.insert(k, 1)),
            2 => assert_eq!(t.eliminar(&k), h.remove(&k)),
            _ => assert_eq!(t.obtener(&k), h.get(&k)),
        }
    }
    assert_eq!(t.len(), h.len());
    let mut a: Vec<_> = t.iter().map(|(k, _)| k.clone()).collect();
    let mut b: Vec<_> = h.keys().cloned().collect();
    a.sort();
    b.sort();
    assert_eq!(a, b);
}

#[test]
fn lista_vs_vec() {
    let mut rng = XorShift::new(3);
    let mut l = Lista::new();
    let mut v: Vec<u64> = Vec::new();
    for _ in 0..10_000 {
        match rng.rango(5) {
            0 | 1 => {
                let x = rng.rango(100);
                l.push(x);
                v.push(x);
            }
            2 => assert_eq!(l.pop(), v.pop()),
            3 => {
                let x = rng.rango(100);
                let pos = v.iter().rposition(|y| *y == x);
                assert_eq!(l.eliminar(&x), pos.is_some());
                if let Some(p) = pos {
                    v.remove(p);
                }
            }
            _ => assert_eq!(l.peek(), v.last()),
        }
    }
    assert!(l.iter().eq(v.iter().rev()));
}
