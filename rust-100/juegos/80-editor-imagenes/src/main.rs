//! Uso: `editor-imagenes ENTRADA.png SALIDA.png comando1 comando2 ...`
//! Si ENTRADA es `--demo`, se genera una imagen sintética.
//! Comandos: grises invertir sepia enfocar bordes rotar espejo-h espejo-v
//!           brillo:N contraste:K umbral:T blur:SIGMA recortar:X,Y,W,H
//!           resize:W,H resize-px:W,H deshacer rehacer

use editor_imagenes::editor::Editor;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (entrada, salida, cmds) = match args.as_slice() {
        [e, s, resto @ ..] => (e.clone(), s.clone(), resto.to_vec()),
        _ => ("--demo".into(), std::env::temp_dir().join("editor-demo.png").display().to_string(), vec!["blur:1.5".into(), "bordes".into(), "invertir".into(), "resize:160,90".into()]),
    };
    let img = if entrada == "--demo" {
        editor_imagenes::imagen_demo(320, 180)
    } else {
        match image::open(&entrada) {
            Ok(i) => i.to_rgba8(),
            Err(e) => {
                eprintln!("No se pudo abrir {entrada}: {e}");
                return ExitCode::FAILURE;
            }
        }
    };
    let mut ed = Editor::new(img);
    for c in &cmds {
        match ed.ejecutar(c) {
            Ok(()) => println!("✓ {c:<16} → {}x{}", ed.imagen.width(), ed.imagen.height()),
            Err(e) => {
                eprintln!("✗ {c}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    if let Err(e) = ed.imagen.save(&salida) {
        eprintln!("No se pudo guardar {salida}: {e}");
        return ExitCode::FAILURE;
    }
    println!("Guardado en {salida}");
    ExitCode::SUCCESS
}
