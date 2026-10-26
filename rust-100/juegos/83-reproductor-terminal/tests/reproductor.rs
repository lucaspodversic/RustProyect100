use ratatui::backend::TestBackend;
use ratatui::Terminal;
use reproductor_terminal::audio::{BackendSimulado, Llamada};
use reproductor_terminal::lista::*;
use reproductor_terminal::reproductor::*;
use reproductor_terminal::ui::dibujar;

fn pistas(n: usize) -> Vec<Pista> {
    (0..n).map(|i| Pista { ruta: format!("p{i}.ogg"), titulo: format!("Tema {i}"), artista: "X".into(), duracion_ms: None }).collect()
}

fn rep(n: usize) -> Reproductor<BackendSimulado> {
    let cat: Vec<(String, u64)> = (0..n).map(|i| (format!("p{i}.ogg"), 10_000)).collect();
    let refs: Vec<(&str, u64)> = cat.iter().map(|(a, b)| (a.as_str(), *b)).collect();
    Reproductor::new(BackendSimulado::con_catalogo(&refs), pistas(n))
}

#[test]
fn m3u_extendido_y_simple() {
    let t = "#EXTM3U\n#EXTINF:125,Artista Uno - Tema Uno\nmusica/uno.mp3\n\n# comentario\ndos.flac\n/abs/tres.ogg\n#EXTINF:-1,Radio\nhttp://radio\n";
    let p = parsear_m3u(t, "/home/yo");
    assert_eq!(p.len(), 4);
    assert_eq!(p[0], Pista { ruta: "/home/yo/musica/uno.mp3".into(), titulo: "Tema Uno".into(), artista: "Artista Uno".into(), duracion_ms: Some(125_000) });
    assert_eq!(p[1].titulo, "dos");
    assert_eq!(p[1].duracion_ms, None);
    assert_eq!(p[2].ruta, "/abs/tres.ogg");
    assert_eq!(p[3].titulo, "Radio");
    assert_eq!(p[3].duracion_ms, None, "-1 = duración desconocida");
    assert_eq!(formato_tiempo(65_000), "1:05");
    assert_eq!(formato_tiempo(3_725_000), "1:02:05");
}

#[test]
fn play_pausa_detener() {
    let mut r = rep(3);
    r.ejecutar(Comando::PlayPausa);
    assert_eq!(r.estado, Estado::Reproduciendo);
    r.tick(4_000);
    assert_eq!(r.posicion(), 4_000);
    r.ejecutar(Comando::PlayPausa);
    assert_eq!(r.estado, Estado::Pausado);
    r.tick(4_000);
    assert_eq!(r.posicion(), 4_000, "en pausa el tiempo no avanza");
    r.ejecutar(Comando::PlayPausa);
    r.ejecutar(Comando::Detener);
    assert_eq!((r.estado, r.posicion()), (Estado::Detenido, 0));
    assert_eq!(r.backend.llamadas[0], Llamada::Cargar("p0.ogg".into()));
}

#[test]
fn avance_automatico_y_fin_de_lista() {
    let mut r = rep(2);
    r.ejecutar(Comando::PlayPausa);
    r.tick(10_000);
    assert_eq!(r.pista_actual().unwrap().titulo, "Tema 1");
    r.tick(10_000);
    assert_eq!(r.estado, Estado::Detenido);
    assert_eq!(r.mensaje, "Fin de la lista");
}

#[test]
fn repetir_todo_y_una() {
    let mut r = rep(2);
    r.ejecutar(Comando::CiclarRepetir);
    assert_eq!(r.repetir, Repetir::Todo);
    r.ejecutar(Comando::PlayPausa);
    r.tick(10_000);
    r.tick(10_000);
    assert_eq!(r.pista_actual().unwrap().titulo, "Tema 0", "vuelve al principio");
    assert_eq!(r.estado, Estado::Reproduciendo);
    r.ejecutar(Comando::CiclarRepetir);
    assert_eq!(r.repetir, Repetir::Una);
    r.tick(10_000);
    assert_eq!(r.pista_actual().unwrap().titulo, "Tema 0");
    assert_eq!(r.posicion(), 0);
    // Con "una", siguiente manual sí avanza.
    r.ejecutar(Comando::Siguiente);
    assert_eq!(r.pista_actual().unwrap().titulo, "Tema 1");
    r.ejecutar(Comando::CiclarRepetir);
    assert_eq!(r.repetir, Repetir::No);
}

#[test]
fn anterior_reinicia_o_retrocede() {
    let mut r = rep(3);
    r.ejecutar(Comando::Elegir(1));
    r.tick(5_000);
    r.ejecutar(Comando::Anterior);
    assert_eq!((r.pista_actual().unwrap().titulo.as_str(), r.posicion()), ("Tema 1", 0), "reinicia");
    r.tick(1_000);
    r.ejecutar(Comando::Anterior);
    assert_eq!(r.pista_actual().unwrap().titulo, "Tema 0");
    r.ejecutar(Comando::Anterior);
    assert_eq!(r.pista_actual().unwrap().titulo, "Tema 2", "da la vuelta");
}

#[test]
fn seek_acotado() {
    let mut r = rep(1);
    r.ejecutar(Comando::Adelantar(5_000));
    assert!(r.backend.llamadas.is_empty(), "detenido: no hace nada");
    r.ejecutar(Comando::PlayPausa);
    r.ejecutar(Comando::Adelantar(7_000));
    assert_eq!(r.posicion(), 7_000);
    r.ejecutar(Comando::Adelantar(7_000));
    assert_eq!(r.posicion(), 10_000);
    r.ejecutar(Comando::Retroceder(60_000));
    assert_eq!(r.posicion(), 0);
}

#[test]
fn volumen_y_mute() {
    let mut r = rep(1);
    r.ejecutar(Comando::PlayPausa);
    for _ in 0..10 {
        r.ejecutar(Comando::SubirVolumen);
    }
    assert_eq!(r.volumen, 100);
    assert_eq!(r.backend.vol, 1.0);
    r.ejecutar(Comando::Silencio);
    assert_eq!(r.backend.vol, 0.0);
    r.ejecutar(Comando::Silencio);
    assert_eq!(r.backend.vol, 1.0, "recupera el nivel");
    for _ in 0..30 {
        r.ejecutar(Comando::BajarVolumen);
    }
    assert_eq!(r.volumen, 0);
}

#[test]
fn aleatorio_mantiene_la_actual_y_es_permutacion() {
    let mut r = rep(8);
    r.ejecutar(Comando::Elegir(3));
    r.ejecutar(Comando::Aleatorio);
    assert_eq!(r.orden[0], 3);
    assert_eq!(r.pista_actual().unwrap().titulo, "Tema 3");
    let mut o = r.orden.clone();
    o.sort();
    assert_eq!(o, (0..8).collect::<Vec<_>>());
    assert_ne!(r.orden, vec![3, 0, 1, 2, 4, 5, 6, 7], "debería mezclar");
    r.ejecutar(Comando::Siguiente);
    let segunda = r.orden[1];
    assert_eq!(r.pista_actual().unwrap().titulo, format!("Tema {segunda}"));
    r.ejecutar(Comando::Aleatorio);
    assert_eq!(r.pista_actual().unwrap().titulo, format!("Tema {segunda}"), "desactivar no cambia lo que suena");
}

#[test]
fn archivos_rotos_se_saltan_y_lista_vacia() {
    let mut r = Reproductor::new(BackendSimulado::con_catalogo(&[("bien.ogg", 1000)]), vec![
        Pista { ruta: "roto.ogg".into(), titulo: "Roto".into(), artista: String::new(), duracion_ms: None },
        Pista { ruta: "bien.ogg".into(), titulo: "Bien".into(), artista: String::new(), duracion_ms: None },
    ]);
    r.ejecutar(Comando::PlayPausa);
    assert_eq!(r.pista_actual().unwrap().titulo, "Bien");
    let mut todo_roto = Reproductor::new(BackendSimulado::default(), pistas(3));
    todo_roto.ejecutar(Comando::PlayPausa);
    assert_eq!(todo_roto.estado, Estado::Detenido, "no cicla infinito");
    let mut vacio = Reproductor::new(BackendSimulado::default(), vec![]);
    vacio.ejecutar(Comando::Siguiente);
    assert_eq!(vacio.mensaje, "Lista vacía");
}

#[test]
fn teclas_y_ui_con_test_backend() {
    assert_eq!(Comando::desde_tecla(' '), Some(Comando::PlayPausa));
    assert_eq!(Comando::desde_tecla('l'), Some(Comando::Adelantar(5_000)));
    assert_eq!(Comando::desde_tecla('q'), None);
    let mut r = rep(3);
    r.ejecutar(Comando::PlayPausa);
    r.tick(5_000);
    let mut t = Terminal::new(TestBackend::new(80, 12)).unwrap();
    t.draw(|f| dibujar(f, &r)).unwrap();
    let buf = t.backend().buffer();
    let txt: String = (0..buf.area.height).flat_map(|y| (0..buf.area.width).map(move |x| (x, y))).map(|(x, y)| buf[(x, y)].symbol().to_string()).collect();
    assert!(txt.contains("♪ Tema 0"));
    assert!(txt.contains("0:05 / 0:10"));
    assert!(txt.contains("Vol 70%"));
}
