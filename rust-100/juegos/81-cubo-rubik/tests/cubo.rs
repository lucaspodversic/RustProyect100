use cubo_rubik::cubo::*;
use cubo_rubik::geometria::{indice, sticker};
use cubo_rubik::resolver::*;

fn tras(s: &str) -> Cubo {
    let mut c = Cubo::resuelto();
    c.ejecutar(s).unwrap();
    c
}

#[test]
fn indices_de_stickers_son_biyectivos() {
    for i in 0..54 {
        let (p, n) = sticker(i);
        assert_eq!(indice(p, n), i);
    }
}

#[test]
fn cada_movimiento_cuatro_veces_es_identidad() {
    for e in Eje::TODOS {
        let mut c = Cubo::resuelto();
        let m = Mov { eje: e, giros: 1 };
        c.aplicar(m);
        assert_ne!(c, Cubo::resuelto(), "{m} debe cambiar algo");
        for _ in 0..3 {
            c.aplicar(m);
        }
        assert_eq!(c, Cubo::resuelto(), "{m}^4");
    }
}

#[test]
fn orientacion_de_los_giros_es_la_estandar() {
    // R: la columna derecha del frente (verde) sube a U.
    let c = tras("R");
    assert_eq!([c.s[2], c.s[5], c.s[8]], [2, 2, 2]);
    // U: la fila superior de F recibe los colores de R (rojo).
    let c = tras("U");
    assert_eq!(&c.s[18..21], &[1, 1, 1]);
    // F: la fila inferior de U recibe el naranja de L.
    let c = tras("F");
    assert_eq!(&c.s[6..9], &[4, 4, 4]);
    // M sigue a L: el frente baja → U recibe el azul de B en el centro.
    assert_eq!(tras("M").s[4], 5);
    // x sigue a R: U pasa a ser el verde del frente.
    assert_eq!(tras("x").s[4], 2);
}

#[test]
fn equivalencias_conocidas() {
    // Rotación x = R M' L'.
    assert_eq!(tras("x"), tras("R M' L'"));
    assert_eq!(tras("y"), tras("U E' D'"));
    assert_eq!(tras("z"), tras("F S B'"));
    // Caras opuestas conmutan.
    assert_eq!(tras("R L"), tras("L R"));
    assert_ne!(tras("R U"), tras("U R"));
}

#[test]
fn ordenes_de_secuencias_clasicas() {
    assert_eq!(orden(&parsear("R U R' U'").unwrap()), 6, "sexy move");
    assert_eq!(orden(&parsear("R U").unwrap()), 105);
    assert_eq!(orden(&parsear("R2 U2").unwrap()), 6);
    assert_eq!(orden(&parsear("R").unwrap()), 4);
}

#[test]
fn inversa_deshace_la_mezcla() {
    let m = mezcla(40, 7);
    let mut c = Cubo::resuelto();
    c.aplicar_seq(&m);
    assert!(!c.esta_resuelto());
    c.aplicar_seq(&inversa(&m));
    assert_eq!(c, Cubo::resuelto());
}

#[test]
fn cada_color_aparece_9_veces_y_centros_fijos() {
    let mut c = Cubo::resuelto();
    c.aplicar_seq(&mezcla(60, 99));
    for color in 0..6u8 {
        assert_eq!(c.s.iter().filter(|&&x| x == color).count(), 9);
    }
    // Los movimientos de cara no mueven centros.
    for cara in 0..6 {
        assert_eq!(c.s[cara * 9 + 4], cara as u8);
    }
}

#[test]
fn mezcla_sin_redundancias_y_reproducible() {
    let a = mezcla(25, 5);
    assert_eq!(a, mezcla(25, 5));
    assert!(a.windows(2).all(|w| w[0].eje != w[1].eje));
}

#[test]
fn notacion_ida_y_vuelta_y_errores() {
    let s = "R U2 F' M E S x y' z2 L D B";
    assert_eq!(a_texto(&parsear(s).unwrap()), s);
    assert!(parsear("R Q").is_err());
    assert!(parsear("R3").is_err());
    assert_eq!(parsear("U2'").unwrap()[0].giros, 2);
}

#[test]
fn resolvedor_encuentra_soluciones_cortas() {
    for semilla in [1, 2, 3] {
        let m = mezcla(4, semilla);
        let mut c = Cubo::resuelto();
        c.aplicar_seq(&m);
        let sol = resolver(&c, 4).expect("≤4 movimientos");
        assert!(sol.len() <= 4);
        c.aplicar_seq(&sol);
        assert!(c.esta_resuelto());
    }
    assert_eq!(resolver(&Cubo::resuelto(), 3), Some(vec![]));
    assert!(resolver(&tras("R U"), 1).is_none());
}

#[test]
fn red_desplegada() {
    let r = Cubo::resuelto().red(false);
    assert_eq!(r.lines().count(), 9);
    assert_eq!(r.lines().nth(3).unwrap(), "OOO GGG RRR BBB");
    assert!(tras("R").red(true).contains("\x1b["));
}
