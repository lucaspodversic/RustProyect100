use editor_imagenes::editor::*;
use editor_imagenes::filtros::*;
use editor_imagenes::imagen_demo;
use image::{Rgba, RgbaImage};

fn liso(w: u32, h: u32, c: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba(c))
}

#[test]
fn filtros_de_color() {
    let img = liso(2, 2, [255, 0, 0, 200]);
    assert_eq!(escala_grises(&img).get_pixel(0, 0).0, [76, 76, 76, 200], "Rec.601 y alfa intacto");
    assert_eq!(invertir(&img).get_pixel(1, 1).0, [0, 255, 255, 200]);
    assert_eq!(brillo(&img, 100).get_pixel(0, 0).0, [255, 100, 100, 200], "satura en 255");
    assert_eq!(contraste(&liso(1, 1, [128, 100, 200, 255]), 2.0).get_pixel(0, 0).0, [128, 72, 255, 255]);
    let s = sepia(&liso(1, 1, [100, 100, 100, 255]));
    let p = s.get_pixel(0, 0);
    assert!(p[0] > p[1] && p[1] > p[2], "tono cálido");
    assert_eq!(umbral(&liso(1, 1, [200, 200, 200, 255]), 128).get_pixel(0, 0)[0], 255);
}

#[test]
fn desenfoque_conserva_color_liso_y_suaviza_bordes() {
    let img = liso(9, 9, [50, 100, 150, 255]);
    assert_eq!(desenfoque(&img, 2.0), img);
    // Un punto blanco se dispersa y conserva aprox. la energía total.
    let mut p = liso(15, 15, [0, 0, 0, 255]);
    p.put_pixel(7, 7, Rgba([255, 255, 255, 255]));
    let b = desenfoque(&p, 1.0);
    assert!(b.get_pixel(7, 7)[0] < 255 && b.get_pixel(8, 7)[0] > 0);
    let total: u32 = b.pixels().map(|q| q[0] as u32).sum();
    assert!((230..=280).contains(&total), "{total}");
}

#[test]
fn bordes_sobel_detecta_la_frontera() {
    let img = RgbaImage::from_fn(10, 10, |x, _| if x < 5 { Rgba([0, 0, 0, 255]) } else { Rgba([255, 255, 255, 255]) });
    let e = bordes(&img);
    assert_eq!(e.get_pixel(1, 5)[0], 0, "zona plana");
    assert_eq!(e.get_pixel(4, 5)[0], 255, "borde vertical");
    let en = enfocar(&liso(4, 4, [90, 90, 90, 255]));
    assert_eq!(en.get_pixel(2, 2)[0], 90, "enfocar no cambia zonas lisas");
}

#[test]
fn recorte_y_limites() {
    let img = imagen_demo(40, 20);
    let r = recortar(&img, 10, 5, 8, 6).unwrap();
    assert_eq!(r.dimensions(), (8, 6));
    assert_eq!(r.get_pixel(0, 0), img.get_pixel(10, 5));
    assert!(recortar(&img, 35, 0, 10, 5).is_none());
    assert!(recortar(&img, 0, 0, 0, 5).is_none());
    assert!(recortar(&img, u32::MAX, 0, 2, 2).is_none(), "sin overflow");
}

#[test]
fn redimensionado() {
    let mut img = liso(2, 2, [0, 0, 0, 255]);
    img.put_pixel(1, 0, Rgba([255, 255, 255, 255]));
    let v = redimensionar_vecino(&img, 4, 4);
    assert_eq!(v.dimensions(), (4, 4));
    assert_eq!(v.get_pixel(3, 0)[0], 255);
    assert_eq!(v.get_pixel(0, 3)[0], 0);
    // Bilineal: valores intermedios en la transición.
    let b = redimensionar_bilineal(&liso(1, 1, [10, 20, 30, 255]), 5, 3);
    assert!(b.pixels().all(|p| p.0 == [10, 20, 30, 255]));
    let fila = RgbaImage::from_fn(2, 1, |x, _| Rgba([(x * 200) as u8, 0, 0, 255]));
    let r = redimensionar_bilineal(&fila, 4, 1);
    let vals: Vec<u8> = (0..4).map(|x| r.get_pixel(x, 0)[0]).collect();
    assert_eq!(vals[0], 0);
    assert_eq!(vals[3], 200);
    assert!(vals[1] > 0 && vals[1] < vals[2] && vals[2] < 200, "{vals:?}");
}

#[test]
fn rotacion_y_espejos() {
    let img = imagen_demo(6, 4);
    let r = rotar90(&img);
    assert_eq!(r.dimensions(), (4, 6));
    assert_eq!(rotar90(&rotar90(&rotar90(&r))), img, "4 rotaciones = identidad");
    assert_eq!(espejo_h(&espejo_h(&img)), img);
    assert_eq!(espejo_v(&img).get_pixel(0, 0), img.get_pixel(0, 3));
    // Rotar 90° horario: la esquina inferior-izquierda pasa arriba-izquierda.
    assert_eq!(r.get_pixel(0, 0), img.get_pixel(0, 3));
}

#[test]
fn parser_de_comandos() {
    assert_eq!(Op::parsear("blur:1.5"), Ok(Op::Desenfoque(1.5)));
    assert_eq!(Op::parsear("recortar:1,2,3,4"), Ok(Op::Recortar(1, 2, 3, 4)));
    assert_eq!(Op::parsear("resize-px:8,8"), Ok(Op::Redimensionar(8, 8, true)));
    assert!(matches!(Op::parsear("pintar"), Err(ErrorEditor::Desconocido(_))));
    assert!(matches!(Op::parsear("recortar:1,2"), Err(ErrorEditor::Argumentos(_))));
    assert!(matches!(Op::parsear("resize:0,5"), Err(ErrorEditor::Argumentos(_))));
    assert!(matches!(Op::parsear("recortar:-1,0,2,2"), Err(ErrorEditor::Argumentos(_))));
    assert!(matches!(Op::parsear("brillo:abc"), Err(ErrorEditor::Argumentos(_))));
    assert_eq!(Op::parsear("blur:1e9"), Ok(Op::Desenfoque(50.0)), "se acota para no colgar");
}

#[test]
fn deshacer_rehacer() {
    let orig = imagen_demo(12, 8);
    let mut ed = Editor::new(orig.clone());
    ed.ejecutar("invertir").unwrap();
    ed.ejecutar("recortar:0,0,4,4").unwrap();
    assert_eq!(ed.imagen.dimensions(), (4, 4));
    ed.ejecutar("deshacer").unwrap();
    ed.ejecutar("deshacer").unwrap();
    assert_eq!(ed.imagen, orig);
    assert_eq!(ed.deshacer(), Err(ErrorEditor::SinHistorial));
    ed.rehacer().unwrap();
    assert_eq!(ed.imagen, invertir(&orig));
    ed.ejecutar("grises").unwrap();
    assert_eq!(ed.niveles_historial().1, 0, "editar borra el rehacer");
    // Error: la imagen no cambia ni se ensucia el historial.
    let antes = ed.niveles_historial();
    assert_eq!(ed.ejecutar("recortar:0,0,999,1"), Err(ErrorEditor::RecorteInvalido));
    assert_eq!(ed.niveles_historial(), antes);
}

#[test]
fn historial_limitado() {
    let mut ed = Editor::new(liso(2, 2, [0, 0, 0, 255]));
    ed.max_historial = 3;
    for _ in 0..10 {
        ed.ejecutar("brillo:10").unwrap();
    }
    assert_eq!(ed.niveles_historial().0, 3);
}

#[test]
fn png_ida_y_vuelta_e_histograma() {
    let img = imagen_demo(32, 16);
    let ruta = std::env::temp_dir().join(format!("editor-test-{}.png", std::process::id()));
    img.save(&ruta).unwrap();
    let leida = image::open(&ruta).unwrap().to_rgba8();
    std::fs::remove_file(&ruta).ok();
    assert_eq!(leida, img, "PNG es sin pérdida");
    let h = histograma(&liso(4, 4, [255, 255, 255, 255]));
    assert_eq!(h[255], 16);
    assert_eq!(h.iter().sum::<u32>(), 16);
}
