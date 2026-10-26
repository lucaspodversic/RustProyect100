use arbol_merkle::*;

#[test]
fn bitcoin_bloque_100000_raiz_real() {
    // Las 4 transacciones del bloque 100.000 de Bitcoin (txids en orden de pantalla).
    let txids = [
        "8c14f0db3df150123e6f3dbbf30f8b955a8249b62ac1d1ff16284aefa3d06d87",
        "fff2525b8931402dd09222c50775608f75787bd2b87e56995a7bdd30f79702c4",
        "6359f0868171b1d194cbee1af2f16ea598ae8fad666d9b012c8ed2b79a236ec4",
        "e9a66845e05d5abc0ad04ec80f774a7e585c6e8db975962d069a522137b80c1d",
    ];
    let hojas = txids.iter().map(|t| desde_hex_invertido(t).unwrap()).collect();
    let a = ArbolMerkle::desde_hojas(Modo::Bitcoin, hojas);
    assert_eq!(a_hex_invertido(&a.raiz()), "f3e94742aca4b5ef85488dc37c06c3282295ffec960994b2c0d5ac2a25a95766");
}

#[test]
fn todas_las_pruebas_verifican_para_varios_tamanos() {
    for modo in [Modo::Rfc6962, Modo::Bitcoin] {
        for n in 1..=33usize {
            let datos: Vec<Vec<u8>> = (0..n).map(|i| format!("tx-{i}").into_bytes()).collect();
            let a = ArbolMerkle::desde_datos(modo, &datos);
            for (i, d) in datos.iter().enumerate() {
                let p = a.prueba(i).unwrap();
                assert!(verificar(modo, &modo.hash_hoja(d), &p, &a.raiz()), "modo {modo:?} n={n} i={i}");
                assert!(p.pasos.len() <= (usize::BITS - n.leading_zeros()) as usize);
            }
            assert!(a.prueba(n).is_none());
        }
    }
}

#[test]
fn prueba_falla_con_hoja_o_raiz_equivocada() {
    let a = ArbolMerkle::desde_datos(Modo::Rfc6962, &["a", "b", "c", "d", "e"]);
    let p = a.prueba(2).unwrap();
    assert!(!verificar(Modo::Rfc6962, &Modo::Rfc6962.hash_hoja(b"x"), &p, &a.raiz()));
    assert!(!verificar(Modo::Rfc6962, &Modo::Rfc6962.hash_hoja(b"c"), &p, &[0; 32]));
    // Prueba de otra posición tampoco sirve.
    assert!(!verificar(Modo::Rfc6962, &Modo::Rfc6962.hash_hoja(b"c"), &a.prueba(3).unwrap(), &a.raiz()));
}

#[test]
fn cambiar_una_tx_cambia_la_raiz() {
    let a = ArbolMerkle::desde_datos(Modo::Rfc6962, &["a", "b", "c"]);
    let b = ArbolMerkle::desde_datos(Modo::Rfc6962, &["a", "B", "c"]);
    assert_ne!(a.raiz(), b.raiz());
    // El orden también importa.
    let c = ArbolMerkle::desde_datos(Modo::Rfc6962, &["b", "a", "c"]);
    assert_ne!(a.raiz(), c.raiz());
}

#[test]
fn bitcoin_cve_2012_2459_y_rfc_inmune() {
    // En modo Bitcoin, [a,b,c] y [a,b,c,c] dan la MISMA raíz (duplicación).
    let b3 = ArbolMerkle::desde_datos(Modo::Bitcoin, &["a", "b", "c"]);
    let b4 = ArbolMerkle::desde_datos(Modo::Bitcoin, &["a", "b", "c", "c"]);
    assert_eq!(b3.raiz(), b4.raiz());
    // En RFC 6962 no.
    let r3 = ArbolMerkle::desde_datos(Modo::Rfc6962, &["a", "b", "c"]);
    let r4 = ArbolMerkle::desde_datos(Modo::Rfc6962, &["a", "b", "c", "c"]);
    assert_ne!(r3.raiz(), r4.raiz());
}

#[test]
fn rfc_resiste_segunda_preimagen() {
    // Ataque: presentar un nodo interno (izq||der) como si fuera una hoja.
    let a = ArbolMerkle::desde_datos(Modo::Rfc6962, &["a", "b", "c", "d"]);
    let interno: Vec<u8> = [a.niveles[0][0], a.niveles[0][1]].concat();
    let falso = ArbolMerkle::desde_datos(Modo::Rfc6962, &[interno, [a.niveles[0][2], a.niveles[0][3]].concat()]);
    assert_ne!(falso.raiz(), a.raiz());
}

#[test]
fn rfc6962_arbol_vacio_y_unitario() {
    let v = ArbolMerkle::desde_datos::<&str>(Modo::Rfc6962, &[]);
    assert_eq!(hex::encode(v.raiz()), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    // Vector RFC 6962: hoja vacía → sha256(0x00).
    let u = ArbolMerkle::desde_datos(Modo::Rfc6962, &[b""]);
    assert_eq!(hex::encode(u.raiz()), "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d");
}

#[test]
fn actualizar_hoja_equivale_a_reconstruir() {
    for modo in [Modo::Rfc6962, Modo::Bitcoin] {
        let mut datos: Vec<String> = (0..11).map(|i| i.to_string()).collect();
        let mut a = ArbolMerkle::desde_datos(modo, &datos);
        datos[7] = "nuevo".into();
        assert!(a.actualizar_hoja(7, modo.hash_hoja(b"nuevo")));
        assert_eq!(a.raiz(), ArbolMerkle::desde_datos(modo, &datos).raiz());
        datos[10] = "fin".into();
        a.actualizar_hoja(10, modo.hash_hoja(b"fin"));
        assert_eq!(a.raiz(), ArbolMerkle::desde_datos(modo, &datos).raiz());
        assert!(!a.actualizar_hoja(99, [0; 32]));
    }
}

#[test]
fn cliente_spv() {
    let bloques: Vec<ArbolMerkle> = (0..3).map(|b| ArbolMerkle::desde_datos(Modo::Rfc6962, &(0..5).map(|i| format!("b{b}t{i}")).collect::<Vec<_>>())).collect();
    let spv = ClienteSpv { raices: bloques.iter().map(|a| a.raiz()).collect() };
    let hoja = Modo::Rfc6962.hash_hoja(b"b1t3");
    let p = bloques[1].prueba(3).unwrap();
    assert!(spv.tx_confirmada(Modo::Rfc6962, 1, &hoja, &p));
    assert!(!spv.tx_confirmada(Modo::Rfc6962, 2, &hoja, &p));
    assert!(!spv.tx_confirmada(Modo::Rfc6962, 9, &hoja, &p));
}

#[test]
fn hex_invertido_roundtrip() {
    let s = "8c14f0db3df150123e6f3dbbf30f8b955a8249b62ac1d1ff16284aefa3d06d87";
    assert_eq!(a_hex_invertido(&desde_hex_invertido(s).unwrap()), s);
    assert!(desde_hex_invertido("abc").is_none());
}
