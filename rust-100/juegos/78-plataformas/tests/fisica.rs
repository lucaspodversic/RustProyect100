use plataformas::fisica::*;
use plataformas::juego::*;
use plataformas::nivel::*;

fn nivel(txt: &str) -> Nivel {
    Nivel::parsear(txt).unwrap()
}

fn simular(j: &mut Juego, e: Entrada, pasos: usize) -> Vec<Evento> {
    (0..pasos).flat_map(|_| j.actualizar(e)).collect()
}

const PLANO: &str = "
#          #
#          #
#  P       #
############";

#[test]
fn parseo_de_niveles() {
    let n = nivel(NIVEL_1);
    assert_eq!(n.inicio, (3.0, 6.0));
    assert!(n.monedas() >= 5);
    assert_eq!(Nivel::parsear("###").unwrap_err(), ErrorNivel::SinInicio);
    assert_eq!(Nivel::parsear("P?").unwrap_err(), ErrorNivel::CaracterInvalido('?'));
    assert_eq!(n.tile(-1, 0), Tile::Solido, "paredes invisibles a los lados");
}

#[test]
fn gravedad_y_aterrizaje() {
    let mut j = Juego::new(nivel(PLANO));
    simular(&mut j, Entrada::default(), 60);
    assert!(j.jugador.en_suelo);
    assert!((j.jugador.y + ALTO_J - 3.0).abs() < 1e-3, "los pies apoyan exactamente en y=3");
    assert_eq!(j.jugador.vy, 0.0);
}

#[test]
fn salto_variable_alto_vs_corto() {
    let altura_max = |mantener: usize| {
        let mut j = Juego::new(nivel("#    #\n#    #\n#    #\n#    #\n#    #\n#    #\n#    #\n#  P #\n######"));
        simular(&mut j, Entrada::default(), 30);
        let y0 = j.jugador.y;
        let mut min_y = y0;
        for i in 0..90 {
            j.actualizar(Entrada { saltar: i < mantener, ..Default::default() });
            min_y = min_y.min(j.jugador.y);
        }
        y0 - min_y
    };
    let (largo, corto) = (altura_max(40), altura_max(2));
    assert!(largo > 3.0, "salto largo: {largo}");
    assert!(corto < largo * 0.5, "soltar antes corta el salto ({corto} vs {largo})");
}

#[test]
fn paredes_bloquean_movimiento() {
    let mut j = Juego::new(nivel(PLANO));
    simular(&mut j, Entrada { der: true, ..Default::default() }, 200);
    assert!((j.jugador.x + ANCHO_J - 11.0).abs() < 1e-3, "x={}", j.jugador.x);
    assert_eq!(j.jugador.vx, 0.0);
}

#[test]
fn coyote_time_permite_saltar_tras_dejar_el_borde() {
    let n = nivel("    \nP   \n#   \n    \n    \n    ");
    let mut c = Cuerpo::new(n.inicio.0, n.inicio.1);
    for _ in 0..30 {
        c.actualizar(&n, Entrada::default(), DT);
    }
    assert!(c.en_suelo);
    // Corremos hacia el vacío y saltamos pocos frames después de caer.
    let mut frames_en_aire = 0;
    while frames_en_aire < 3 {
        c.actualizar(&n, Entrada { der: true, ..Default::default() }, DT);
        if !c.en_suelo {
            frames_en_aire += 1;
        }
    }
    c.actualizar(&n, Entrada { saltar: true, der: true, ..Default::default() }, DT);
    assert!(c.vy < 0.0, "saltó gracias al coyote time");
}

#[test]
fn jump_buffer() {
    let mut c = Cuerpo::new(1.0, 0.0);
    let n = nivel("P   \n    \n    \n####");
    // Cae; aprieta saltar justo antes de tocar el piso y lo mantiene.
    let mut salto = false;
    for i in 0..60 {
        let cerca = c.y + ALTO_J > 2.6;
        c.actualizar(&n, Entrada { saltar: cerca, ..Default::default() }, DT);
        if cerca && c.vy < 0.0 {
            salto = true;
            break;
        }
        assert!(i < 59, "y={} vy={} suelo={}", c.y, c.vy, c.en_suelo);
    }
    assert!(salto, "el salto pedido en el aire se ejecutó al aterrizar");
}

#[test]
fn plataforma_un_sentido() {
    let n = nivel("#   #\n#   #\n# = #\n#   #\n# P #\n#####");
    let mut c = Cuerpo::new(2.0, 4.0);
    for _ in 0..10 {
        c.actualizar(&n, Entrada::default(), DT);
    }
    // Salta y atraviesa desde abajo, luego aterriza ARRIBA de la plataforma.
    for i in 0..120 {
        c.actualizar(&n, Entrada { saltar: i < 30, ..Default::default() }, DT);
    }
    assert!(c.en_suelo);
    assert!((c.y + ALTO_J - 2.0).abs() < 1e-3, "parado sobre '=' (y={})", c.y);
    // Abajo + saltar: la atraviesa hacia abajo.
    c.actualizar(&n, Entrada { saltar: true, abajo: true, ..Default::default() }, DT);
    for _ in 0..60 {
        c.actualizar(&n, Entrada::default(), DT);
    }
    assert!((c.y + ALTO_J - 5.0).abs() < 1e-3, "volvió al piso (y={})", c.y);
}

#[test]
fn sin_tunneling_a_alta_velocidad() {
    let n = nivel("P\n \n \n \n \n#");
    let mut c = Cuerpo::new(0.0, 0.0);
    c.vy = 29.0;
    c.actualizar(&n, Entrada::default(), 0.2); // desplazamiento de ~6 celdas en un frame
    assert!(c.y + ALTO_J <= 5.0 + 1e-3, "no atravesó el piso");
}

#[test]
fn monedas_pinchos_vidas_y_meta() {
    let mut j = Juego::new(nivel("#      #\n#P o ^F#\n########"));
    let ev = simular(&mut j, Entrada { der: true, ..Default::default() }, 120);
    assert!(ev.contains(&Evento::Moneda));
    assert!(ev.contains(&Evento::Murio), "los pinchos matan");
    assert!(j.vidas < 3);
    // Saltando sobre los pinchos llega a la meta.
    let mut j = Juego::new(nivel("#        #\n#        #\n#        #\n#P  ^^  F#\n##########"));
    let mut ev = vec![];
    for i in 0..400 {
        ev.extend(j.actualizar(Entrada { der: true, saltar: (i % 40) < 25, ..Default::default() }));
        if j.estado != Estado::Jugando {
            break;
        }
    }
    assert_eq!(j.estado, Estado::Gano, "eventos: {ev:?}");
}

#[test]
fn caer_al_vacio_quita_vidas_hasta_perder() {
    let mut j = Juego::new(nivel("P\n "));
    simular(&mut j, Entrada::default(), 600);
    assert_eq!(j.estado, Estado::Perdio);
    assert_eq!(j.vidas, 0);
}

#[test]
fn camara_y_render() {
    let mut j = Juego::new(nivel(NIVEL_1));
    assert_eq!(j.camara_x(20), 0);
    j.jugador.x = 39.0;
    assert_eq!(j.camara_x(20), 42 - 20);
    let r = j.renderizar(20);
    assert!(r.contains('@'));
    assert!(r.lines().next().unwrap().chars().count() == 20);
}
