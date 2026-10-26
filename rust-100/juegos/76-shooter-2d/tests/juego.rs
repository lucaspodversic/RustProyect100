use shooter_2d::juego::*;
use shooter_2d::motor::*;

#[test]
fn aabb_colisiones() {
    let a = Aabb { pos: V2::new(0.0, 0.0), tam: V2::new(2.0, 2.0) };
    assert!(a.choca(&Aabb { pos: V2::new(1.0, 1.0), tam: V2::new(2.0, 2.0) }));
    assert!(!a.choca(&Aabb { pos: V2::new(2.0, 0.0), tam: V2::new(1.0, 1.0) }), "tocarse en el borde no es choque");
}

#[test]
fn jugador_se_mueve_y_respeta_bordes() {
    let mut j = Juego::new(1);
    let x0 = j.jugador.pos.x;
    j.actualizar(Entrada { derecha: true, ..Default::default() });
    assert!(j.jugador.pos.x > x0);
    for _ in 0..200 {
        j.actualizar(Entrada { izquierda: true, ..Default::default() });
    }
    assert_eq!(j.jugador.pos.x, 0.0);
}

#[test]
fn disparo_con_recarga() {
    let mut j = Juego::new(1);
    j.enemigos.clear();
    j.actualizar(Entrada { disparar: true, ..Default::default() });
    let n = j.balas.iter().filter(|b| b.del_jugador).count();
    j.actualizar(Entrada { disparar: true, ..Default::default() });
    assert_eq!(j.balas.iter().filter(|b| b.del_jugador).count(), n, "la recarga impide el segundo disparo");
    assert_eq!(n, 1);
}

#[test]
fn bala_destruye_enemigo_y_suma_puntos() {
    let mut j = Juego::new(1);
    j.oleada = 1;
    let x = j.jugador.pos.x + 1.0;
    j.enemigos = vec![Enemigo { caja: Aabb { pos: V2::new(x - 0.5, 5.0), tam: V2::new(2.0, 1.0) }, vel: V2::default(), vida: 1, puntos: 77 }];
    let mut ev = vec![];
    for _ in 0..40 {
        ev.extend(j.actualizar(Entrada { disparar: true, ..Default::default() }));
    }
    assert!(ev.contains(&EventoJuego::EnemigoDestruido { puntos: 77 }));
    assert!(j.puntaje >= 77);
}

#[test]
fn danio_invulnerabilidad_y_game_over() {
    let mut j = Juego::new(1);
    j.oleada = 1;
    let bala_enemiga = |j: &Juego| Bala { caja: j.jugador, vel: V2::default(), del_jugador: false };
    j.enemigos = vec![Enemigo { caja: Aabb { pos: V2::new(0.0, 0.0), tam: V2::new(1.0, 1.0) }, vel: V2::default(), vida: 9, puntos: 0 }];
    j.balas.push(bala_enemiga(&j));
    j.actualizar(Entrada::default());
    assert_eq!(j.vidas, 2);
    // Durante la invulnerabilidad no hay más daño.
    j.balas.push(bala_enemiga(&j));
    j.actualizar(Entrada::default());
    assert_eq!(j.vidas, 2);
    for _ in 0..2 {
        j.invulnerable = 0.0;
        j.balas.push(bala_enemiga(&j));
        j.actualizar(Entrada::default());
    }
    assert_eq!(j.vidas, 0);
    assert!(j.terminado);
    let mut fb = Framebuffer::new(40, 24);
    j.renderizar(&mut fb);
    assert!(fb.a_texto().contains("GAME OVER"));
}

#[test]
fn simulacion_determinista_con_misma_semilla() {
    let correr = |s| {
        let mut j = Juego::new(s);
        for _ in 0..900 {
            let e = piloto_automatico(&j);
            j.actualizar(e);
        }
        (j.puntaje, j.vidas, j.oleada, j.enemigos.len())
    };
    assert_eq!(correr(7), correr(7));
}

#[test]
fn piloto_automatico_progresa() {
    let mut j = Juego::new(3);
    for _ in 0..1800 {
        let e = piloto_automatico(&j);
        j.actualizar(e);
    }
    assert!(j.puntaje > 0, "la IA debería destruir enemigos");
    assert!(j.oleada >= 1);
}

#[test]
fn render_en_framebuffer() {
    let mut j = Juego::new(1);
    j.actualizar(Entrada::default());
    let mut fb = Framebuffer::new(40, 24);
    j.renderizar(&mut fb);
    let txt = fb.a_texto();
    assert!(txt.contains("/A\\"));
    assert!(txt.starts_with("PTS 0 VIDAS 3 OLA 1"));
    assert!(txt.contains('W') || txt.contains('w'));
    assert_eq!(txt.lines().count(), 24);
    assert!(fb.a_ansi().contains("\x1b[38;2;"));
}
