//! Tests de integración: protocolo en memoria y sobre TCP real.

use nodo_p2p::red::{difundir, sincronizar_con, Servidor};
use nodo_p2p::{sincronizar_en_memoria, Bloque, Mensaje, Nodo, Recepcion};
use std::time::Duration;

#[test]
fn genesis_identico_en_todos_los_nodos() {
    assert_eq!(Nodo::nuevo("a").bloques[0], Nodo::nuevo("b").bloques[0]);
}

#[test]
fn mensaje_roundtrip_json() {
    let m = Mensaje::Hola { altura: 3, direccion: Some("127.0.0.1:9".into()) };
    let l = m.a_linea();
    assert!(l.ends_with('\n'));
    assert_eq!(Mensaje::desde_linea(&l).unwrap(), m);
}

#[test]
fn nodo_atrasado_se_pone_al_dia_en_memoria() {
    let mut a = Nodo::nuevo("a");
    let mut b = Nodo::nuevo("b");
    for i in 1..=7 {
        a.minar(format!("tx{i}"), i);
    }
    sincronizar_en_memoria(&mut b, &mut a); // b saluda a a
    assert_eq!(b.altura(), 7);
    assert_eq!(a.bloques, b.bloques);
}

#[test]
fn nodo_adelantado_empuja_al_otro() {
    let mut a = Nodo::nuevo("a");
    let mut b = Nodo::nuevo("b");
    a.minar("x", 1);
    a.minar("y", 2);
    // a saluda a b: b responde que va atrás; a no pide nada, pero b sí pide.
    sincronizar_en_memoria(&mut a, &mut b);
    assert_eq!(b.altura(), 2);
}

#[test]
fn bloque_duplicado_e_invalido() {
    let mut a = Nodo::nuevo("a");
    let Mensaje::NuevoBloque(b) = a.minar("x", 1) else { panic!() };
    let mut otro = Nodo::nuevo("o");
    assert_eq!(otro.recibir_bloque(b.clone()), Recepcion::Agregado);
    assert_eq!(otro.recibir_bloque(b.clone()), Recepcion::Duplicado);
    let mut falso = b.clone();
    falso.datos = "falso".into();
    assert_eq!(otro.recibir_bloque(falso), Recepcion::Rechazado);
}

#[test]
fn huerfanos_se_conectan_al_llegar_el_padre() {
    let mut a = Nodo::nuevo("a");
    a.minar("1", 1);
    a.minar("2", 2);
    a.minar("3", 3);
    let mut b = Nodo::nuevo("b");
    // Llegan desordenados: 3, 2, 1.
    assert_eq!(b.recibir_bloque(a.bloques[3].clone()), Recepcion::Adelantado);
    assert_eq!(b.recibir_bloque(a.bloques[2].clone()), Recepcion::Adelantado);
    assert_eq!(b.recibir_bloque(a.bloques[1].clone()), Recepcion::Agregado);
    assert_eq!(b.altura(), 3);
    assert!(b.huerfanos.is_empty());
}

#[test]
fn bifurcacion_se_resuelve_por_cadena_mas_larga() {
    let mut a = Nodo::nuevo("a");
    let mut b = Nodo::nuevo("b");
    a.minar("comun", 1);
    sincronizar_en_memoria(&mut b, &mut a);
    // Bifurcación: cada uno mina su propia rama; la de a es más larga.
    a.minar("rama-a-1", 2);
    a.minar("rama-a-2", 3);
    b.minar("rama-b-1", 2);
    sincronizar_en_memoria(&mut b, &mut a);
    assert_eq!(b.altura(), 3);
    assert_eq!(b.bloques, a.bloques);
}

#[test]
fn tramo_invalido_no_se_adopta() {
    let mut a = Nodo::nuevo("a");
    a.minar("1", 1);
    a.minar("2", 2);
    let mut tramo: Vec<Bloque> = a.bloques[1..].to_vec();
    tramo[1].datos = "manipulado".into();
    let mut b = Nodo::nuevo("b");
    assert!(!b.recibir_tramo(tramo));
    assert_eq!(b.altura(), 0);
}

#[test]
fn descubrimiento_de_pares() {
    let mut a = Nodo::nuevo("a");
    a.procesar(Mensaje::Hola { altura: 0, direccion: Some("10.0.0.1:1".into()) });
    let r = a.procesar(Mensaje::PedirPares);
    assert_eq!(r, vec![Mensaje::Pares(vec!["10.0.0.1:1".into()])]);
}

#[test]
fn sincronizacion_tcp_real_en_localhost() {
    let a = Servidor::iniciar(Nodo::nuevo("A")).unwrap();
    for i in 1..=4 {
        a.nodo.lock().unwrap().minar(format!("tx{i}"), i);
    }
    let b = Servidor::iniciar(Nodo::nuevo("B")).unwrap();
    sincronizar_con(&b.nodo, a.direccion, Some(b.direccion.to_string())).unwrap();
    assert_eq!(b.altura(), 4);
    // A aprendió la dirección de B gracias al saludo.
    assert!(a.nodo.lock().unwrap().pares.contains(&b.direccion.to_string()));
}

#[test]
fn gossip_tcp_de_nuevo_bloque() {
    let a = Servidor::iniciar(Nodo::nuevo("A")).unwrap();
    let b = Servidor::iniciar(Nodo::nuevo("B")).unwrap();
    let anuncio = a.nodo.lock().unwrap().minar("nuevo", 1);
    assert_eq!(difundir(&anuncio, &[b.direccion]), 1);
    // Esperamos a que el hilo del servidor procese.
    for _ in 0..100 {
        if b.altura() == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(b.altura(), 1);
}
