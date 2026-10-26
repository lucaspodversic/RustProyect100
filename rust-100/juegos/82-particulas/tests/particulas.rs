use simulacion_particulas::grilla::Grilla;
use simulacion_particulas::sistema::*;
use simulacion_particulas::vec2::{v2, V2};

#[test]
fn caida_libre_coincide_con_la_formula() {
    let mut s = Sistema::new(100.0, 1000.0, 1);
    s.fuerzas.push(Fuerza::Gravedad(v2(0.0, -10.0)));
    s.agregar(v2(50.0, 900.0), V2::default());
    let dt = 0.001;
    for _ in 0..2000 {
        s.paso(dt);
    }
    // y = y0 − g t²/2 = 900 − 20 = 880 (Euler semi-implícito: error O(dt)).
    let p = &s.particulas[0];
    assert!((p.pos.y - 880.0).abs() < 0.05, "{}", p.pos.y);
    assert!((p.vel.y + 20.0).abs() < 1e-3);
}

#[test]
fn orbita_estable_con_euler_semi_implicito() {
    let mut s = Sistema::new(100.0, 100.0, 1);
    let c = v2(50.0, 50.0);
    s.fuerzas.push(Fuerza::Atractor { centro: c, k: 100.0, eps: 0.0 });
    // Velocidad circular: v = sqrt(k / r) con r = 10.
    s.agregar(v2(60.0, 50.0), v2(0.0, (100.0f32 / 10.0).sqrt()));
    for _ in 0..20_000 {
        s.paso(0.001); // ≈ 1 vuelta (T = 2πr/v ≈ 19.9 s)... 20 s
    }
    let r = (s.particulas[0].pos - c).len();
    assert!((r - 10.0).abs() < 0.2, "radio derivó a {r}");
}

#[test]
fn arrastre_frena_y_viento_depende_de_la_masa() {
    let mut s = Sistema::new(1000.0, 1000.0, 1);
    s.fuerzas.push(Fuerza::Arrastre(1.0));
    s.agregar(v2(500.0, 500.0), v2(10.0, 0.0));
    for _ in 0..100 {
        s.paso(0.01);
    }
    // v(t) = v0·e^(−t) → 10·e^(−1) ≈ 3.68
    assert!((s.particulas[0].vel.x - 3.68).abs() < 0.05);
    let mut s = Sistema::new(1000.0, 1000.0, 1);
    s.fuerzas.push(Fuerza::Viento(v2(2.0, 0.0)));
    s.agregar(v2(500.0, 500.0), V2::default());
    s.agregar(v2(500.0, 500.0), V2::default());
    s.particulas[1].masa = 4.0;
    s.paso(1.0);
    assert!((s.particulas[0].vel.x - 2.0).abs() < 1e-5);
    assert!((s.particulas[1].vel.x - 0.5).abs() < 1e-5);
}

#[test]
fn paredes_contienen_y_rebote_elastico_conserva_energia() {
    let mut s = Sistema::new(10.0, 10.0, 3);
    s.restitucion = 1.0;
    for i in 0..20 {
        s.agregar(v2(1.0 + (i % 5) as f32 * 2.0, 1.0 + (i / 5) as f32 * 2.0), v2((i as f32).sin() * 7.0, (i as f32).cos() * 7.0));
    }
    let e0 = s.energia_cinetica();
    for _ in 0..2000 {
        s.paso(0.005);
    }
    assert!(s.particulas.iter().all(|p| p.pos.x >= p.radio && p.pos.x <= 10.0 - p.radio && p.pos.y >= p.radio && p.pos.y <= 10.0 - p.radio));
    assert!((s.energia_cinetica() - e0).abs() / e0 < 1e-4);
    // Con restitución < 1 se pierde energía.
    s.restitucion = 0.5;
    for _ in 0..2000 {
        s.paso(0.005);
    }
    assert!(s.energia_cinetica() < e0 * 0.5);
}

#[test]
fn choque_elastico_conserva_momento_y_energia() {
    let mut ps = vec![
        Particula { pos: v2(0.0, 0.0), vel: v2(3.0, 1.0), masa: 1.0, radio: 0.5, vida: 1.0 },
        Particula { pos: v2(0.8, 0.3), vel: v2(-1.0, 0.0), masa: 3.0, radio: 0.5, vida: 1.0 },
    ];
    let mom = |ps: &[Particula]| ps.iter().fold(V2::default(), |a, p| a + p.vel * p.masa);
    let ene = |ps: &[Particula]| ps.iter().map(|p| 0.5 * p.masa * p.vel.len2()).sum::<f32>();
    let (m0, e0) = (mom(&ps), ene(&ps));
    colision_elastica(&mut ps, 0, 1);
    assert!((mom(&ps) - m0).len() < 1e-4);
    assert!((ene(&ps) - e0).abs() < 1e-4);
    assert!((ps[1].pos - ps[0].pos).len() >= 1.0 - 1e-4, "sin solapamiento");
    // Masas iguales de frente: intercambian velocidades.
    let mut ps = vec![
        Particula { pos: v2(0.0, 0.0), vel: v2(2.0, 0.0), masa: 1.0, radio: 0.5, vida: 1.0 },
        Particula { pos: v2(0.9, 0.0), vel: v2(0.0, 0.0), masa: 1.0, radio: 0.5, vida: 1.0 },
    ];
    colision_elastica(&mut ps, 0, 1);
    assert!(ps[0].vel.x.abs() < 1e-5 && (ps[1].vel.x - 2.0).abs() < 1e-5);
}

#[test]
fn grilla_encuentra_los_mismos_pares_que_fuerza_bruta() {
    let mut rng = 12345u64;
    let mut azar = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng % 10_000) as f32 / 100.0
    };
    let pos: Vec<V2> = (0..400).map(|_| v2(azar(), azar())).collect();
    let mut g = Grilla::new(3.0);
    g.reconstruir(pos.iter().copied());
    let mut rapido = g.pares(&pos, 3.0);
    rapido.sort();
    let mut bruto = vec![];
    for i in 0..pos.len() {
        for j in i + 1..pos.len() {
            if (pos[i] - pos[j]).len2() < 9.0 {
                bruto.push((i, j));
            }
        }
    }
    assert!(!bruto.is_empty());
    assert_eq!(rapido, bruto);
}

#[test]
fn emisor_respeta_tasa_vida_y_maximo() {
    let mut s = Sistema::new(50.0, 50.0, 9);
    let mut e = Emisor::new(v2(25.0, 25.0), 100.0, 0.0, 3.2);
    e.vida = (0.5, 0.5);
    s.emisores.push(e);
    for _ in 0..30 {
        s.paso(0.01); // 0.3 s → 30 partículas, ninguna vencida
    }
    assert!((29..=31).contains(&s.particulas.len()), "{}", s.particulas.len());
    for _ in 0..100 {
        s.paso(0.01);
    }
    // Estado estacionario ≈ tasa × vida = 50.
    assert!((45..=55).contains(&s.particulas.len()), "{}", s.particulas.len());
    s.max_particulas = 10;
    s.particulas.truncate(5);
    s.emisores[0].tasa = 10_000.0;
    s.paso(0.01);
    assert!(s.particulas.len() <= 10);
}

#[test]
fn colisiones_en_el_sistema_conservan_momento() {
    let mut s = Sistema::new(1000.0, 1000.0, 5);
    s.colisiones = true;
    for i in 0..10 {
        s.agregar(v2(400.0 + i as f32 * 0.4, 500.0), v2(if i % 2 == 0 { 3.0 } else { -2.0 }, 0.5 * i as f32));
    }
    let m0 = s.momento();
    for _ in 0..200 {
        s.paso(0.01);
    }
    assert!((s.momento() - m0).len() < 1e-3);
}

#[test]
fn determinismo_y_render() {
    let correr = || {
        let mut s = Sistema::new(30.0, 10.0, 77);
        s.fuerzas.push(Fuerza::Gravedad(v2(0.0, -9.8)));
        s.emisores.push(Emisor::new(v2(15.0, 1.0), 200.0, 1.57, 0.5));
        for _ in 0..60 {
            s.paso(1.0 / 60.0);
        }
        s.render(30, 10)
    };
    let r = correr();
    assert_eq!(r, correr());
    assert_eq!(r.lines().count(), 10);
    assert!(r.chars().any(|c| "@#*:.".contains(c)));
}
