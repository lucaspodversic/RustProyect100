use snake_highscore::ia::decidir;
use snake_highscore::juego::*;
use snake_highscore::records::*;
use std::collections::VecDeque;

fn tmp(nombre: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("snake-test-{}-{nombre}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn avanza_y_no_permite_giro_180() {
    let mut s = Snake::new(10, 10, Modo::Paredes, 1);
    s.comida = Some((0, 0));
    let cab = s.cuerpo[0];
    s.girar(Dir::Izq); // opuesto a Der: ignorado
    assert_eq!(s.paso(), Resultado::Sigue);
    assert_eq!(s.cuerpo[0], (cab.0 + 1, cab.1));
    assert_eq!(s.cuerpo.len(), 3);
}

#[test]
fn cola_de_giros_rapidos() {
    let mut s = Snake::new(10, 10, Modo::Paredes, 1);
    s.comida = Some((0, 0));
    // Arriba e Izquierda en el mismo tick: ambos se aplican en pasos sucesivos.
    s.girar(Dir::Arriba);
    s.girar(Dir::Izq);
    let (x, y) = s.cuerpo[0];
    s.paso();
    s.paso();
    assert_eq!(s.cuerpo[0], (x - 1, y - 1));
}

#[test]
fn comer_crece_y_suma() {
    let mut s = Snake::new(10, 10, Modo::Paredes, 1);
    let (x, y) = s.cuerpo[0];
    s.comida = Some((x + 1, y));
    assert_eq!(s.paso(), Resultado::Comio);
    assert_eq!((s.cuerpo.len(), s.puntaje), (4, 1));
    let c = s.comida.unwrap();
    assert!(!s.cuerpo.contains(&c), "la comida nueva no cae sobre la serpiente");
}

#[test]
fn muere_contra_pared_o_vuelve_en_toroide() {
    let mut s = Snake::new(6, 3, Modo::Paredes, 1);
    s.comida = Some((0, 0));
    let mut r = Resultado::Sigue;
    for _ in 0..5 {
        r = s.paso();
    }
    assert_eq!(r, Resultado::Murio);
    let mut t = Snake::new(6, 3, Modo::Toroide, 1);
    t.comida = Some((0, 0));
    for _ in 0..3 {
        assert_ne!(t.paso(), Resultado::Murio);
    }
    assert_eq!(t.cuerpo[0].0, 0, "salió por la derecha y entró por la izquierda");
}

#[test]
fn choque_con_el_cuerpo_pero_perseguir_la_cola_es_legal() {
    let mut s = Snake::new(10, 10, Modo::Paredes, 1);
    s.comida = Some((9, 9));
    // Serpiente en cuadrado 2x2: moverse a la celda de la cola es válido.
    s.cuerpo = VecDeque::from([(5, 5), (5, 6), (4, 6), (4, 5)]);
    s.dir = Dir::Arriba;
    s.girar(Dir::Izq);
    assert_eq!(s.paso(), Resultado::Sigue);
    // Ahora una serpiente larga que se muerde.
    s.cuerpo = VecDeque::from([(5, 5), (5, 6), (4, 6), (4, 5), (4, 4), (5, 4), (6, 4)]);
    s.dir = Dir::Izq;
    s.girar(Dir::Arriba);
    s.paso();
    s.girar(Dir::Der);
    assert_eq!(s.paso(), Resultado::Murio);
    assert!(!s.vivo);
}

#[test]
fn ia_juega_bien_y_es_determinista() {
    let jugar = |sem| {
        let mut s = Snake::new(12, 10, Modo::Paredes, sem);
        while s.vivo && s.ticks < 4000 {
            let d = decidir(&s);
            s.girar(d);
            if s.paso() == Resultado::Gano {
                break;
            }
        }
        s.puntaje
    };
    let p = jugar(99);
    assert!(p >= 15, "la IA debería comer bastante (comió {p})");
    assert_eq!(p, jugar(99));
}

#[test]
fn render_texto() {
    let s = Snake::new(8, 4, Modo::Paredes, 1);
    let r = s.renderizar();
    assert!(r.contains('@') && r.contains('*'));
    assert_eq!(r.lines().count(), 4 + 3);
}

#[test]
fn tabla_top10_ordenada() {
    let mut t = Tabla::default();
    assert_eq!(t.agregar("x", 0, 0), None, "puntaje 0 no entra");
    for i in 1..=12 {
        t.agregar(&format!("j{i}"), i * 10, i as u64);
    }
    assert_eq!(t.entradas.len(), MAX_ENTRADAS);
    assert_eq!(t.mejor(), 120);
    assert_eq!(t.entradas.last().unwrap().puntaje, 30);
    assert_eq!(t.agregar("lento", 25, 0), None);
    assert_eq!(t.agregar("empate", 120, 99), Some(1), "empate: el anterior queda primero");
    assert_eq!(t.agregar("mal\nnombre-larguisimo-de-mas", 500, 0), Some(0));
    assert_eq!(t.entradas[0].nombre, "malnombre-largui");
}

#[test]
fn persistencia_atomica_y_recuperacion_de_corrupcion() {
    let dir = tmp("persist");
    let alm = Almacen::new(dir.join("sub/records.json"));
    assert_eq!(alm.cargar(), Tabla::default());
    let mut t = Tabla::default();
    t.agregar("ana", 42, 1);
    alm.guardar(&t).unwrap();
    assert_eq!(Almacen::new(&alm.ruta).cargar(), t, "se recarga igual en otra instancia");
    assert!(!alm.ruta.with_extension("json.tmp").exists());
    std::fs::write(&alm.ruta, "{no es json").unwrap();
    assert_eq!(alm.cargar(), Tabla::default());
    assert!(alm.ruta.with_extension("json.corrupto").exists(), "se respaldó el archivo dañado");
    let _ = std::fs::remove_dir_all(dir);
}
