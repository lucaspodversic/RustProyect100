use visualizador_audio::fft::*;
use visualizador_audio::fuentes::*;
use visualizador_audio::visual::*;

#[test]
fn fft_coincide_con_dft() {
    let x: Vec<Complejo> = (0..64).map(|i| Complejo::new(((i * 7) % 13) as f32 - 6.0, (i % 5) as f32)).collect();
    let mut y = x.clone();
    fft(&mut y);
    let d = dft(&x);
    for (a, b) in y.iter().zip(&d) {
        assert!((a.re - b.re).abs() < 1e-2 && (a.im - b.im).abs() < 1e-2);
    }
}

#[test]
fn parseval_energia_conservada() {
    let x: Vec<Complejo> = (0..256).map(|i| Complejo::new((i as f32 * 0.37).sin(), 0.0)).collect();
    let e_t: f32 = x.iter().map(|c| c.re * c.re).sum();
    let mut y = x;
    fft(&mut y);
    let e_f: f32 = y.iter().map(|c| c.modulo().powi(2)).sum::<f32>() / 256.0;
    assert!((e_t - e_f).abs() / e_t < 1e-3);
}

#[test]
fn seno_aparece_en_su_bin_con_amplitud_correcta() {
    let (sr, n) = (8000, 1024);
    let f = 1000.0; // bin exacto: 1000 / (8000/1024) = 128
    let mut s = Senos::new(sr, vec![(f, 0.5)]);
    let mut buf = vec![0.0; n];
    s.leer(&mut buf);
    let e = espectro(&buf);
    let (pico, &amp) = e.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap();
    assert_eq!(pico, 128);
    assert!((amp - 0.5).abs() < 0.02, "amplitud {amp}");
}

#[test]
fn bandas_logaritmicas_cubren_el_rango() {
    let l = limites_log(44_100, 2048, 24, 40.0, 16_000.0);
    assert_eq!(l.len(), 24);
    for w in l.windows(2) {
        assert_eq!(w[0].1, w[1].0, "contiguas");
    }
    assert!(l.iter().all(|(a, b)| b > a), "al menos un bin por banda");
    // Las bandas altas son más anchas (escala log).
    assert!(l[23].1 - l[23].0 > l[0].1 - l[0].0);
}

#[test]
fn tono_grave_y_agudo_activan_bandas_distintas() {
    let banda = |f: f32| {
        let mut v = Visualizador::new(44_100, 2048, 16);
        let mut s = Senos::new(44_100, vec![(f, 0.8)]);
        let mut b = vec![0.0; 2048];
        for _ in 0..5 {
            s.leer(&mut b);
            v.procesar(&b);
        }
        let i = v.banda_dominante();
        (i, v.centros()[i])
    };
    let (bg, fg) = banda(110.0);
    let (ba, fa) = banda(6000.0);
    assert!(bg < ba);
    assert!((fg / 110.0).log2().abs() < 0.6, "centro {fg}");
    assert!((fa / 6000.0).log2().abs() < 0.6, "centro {fa}");
}

#[test]
fn suavizado_sube_rapido_y_baja_lento_con_picos() {
    let mut v = Visualizador::new(8000, 512, 8);
    let mut s = Senos::new(8000, vec![(1000.0, 1.0)]);
    let mut b = vec![0.0; 512];
    s.leer(&mut b);
    v.procesar(&b);
    let i = v.banda_dominante();
    let alto = v.niveles[i];
    assert!(alto > 0.7);
    v.procesar(&vec![0.0; 512]); // silencio
    assert!(v.niveles[i] < alto && v.niveles[i] > alto * 0.5, "baja despacio");
    assert!(v.picos[i] >= v.niveles[i], "el pico queda arriba");
}

#[test]
fn silencio_y_ruido() {
    let mut v = Visualizador::new(8000, 256, 8);
    v.procesar(&vec![0.0; 256]);
    assert!(v.niveles.iter().all(|n| *n == 0.0));
    let mut r = Ruido::new(8000, 3);
    let mut b = vec![0.0; 256];
    for _ in 0..10 {
        r.leer(&mut b);
        v.procesar(&b);
    }
    // El ruido blanco activa todas las bandas.
    assert!(v.niveles.iter().all(|n| *n > 0.2), "{:?}", v.niveles);
}

#[test]
fn chirp_barre_de_grave_a_agudo() {
    let mut c = Chirp::new(16_000, 200.0, 6000.0, 1.0);
    let mut v = Visualizador::new(16_000, 1024, 16);
    let mut b = vec![0.0; 1024];
    let mut dominantes = vec![];
    while c.leer(&mut b) == 1024 {
        v.procesar(&b);
        dominantes.push(v.banda_dominante());
    }
    assert!(dominantes.last() > dominantes.first());
}

#[test]
fn wav_ida_y_vuelta_y_errores() {
    let m: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.05).sin() * 0.9).collect();
    let bytes = escribir_wav(22_050, &m);
    assert_eq!(&bytes[0..4], b"RIFF");
    let (sr, leido) = leer_wav(&bytes).unwrap();
    assert_eq!((sr, leido.len()), (22_050, 1000));
    assert!(m.iter().zip(&leido).all(|(a, b)| (a - b).abs() < 1e-3));
    assert!(leer_wav(b"hola").is_err());
    assert!(leer_wav(&bytes[..50]).is_err(), "truncado");
}

#[test]
fn renders() {
    let mut v = Visualizador::new(8000, 512, 10);
    let mut s = Senos::new(8000, vec![(500.0, 1.0), (2500.0, 0.5)]);
    let mut b = vec![0.0; 512];
    s.leer(&mut b);
    v.procesar(&b);
    let t = v.render_texto(6);
    assert_eq!(t.lines().count(), 6);
    assert!(t.contains('█'));
    let ppm = v.render_ppm(4, 50);
    assert!(ppm.starts_with(b"P6\n40 50\n255\n"));
    assert_eq!(ppm.len(), "P6\n40 50\n255\n".len() + 40 * 50 * 3);
}
