//! Demo: visualiza un chirp 100 Hz → 8 kHz, o un WAV: `cargo run -- archivo.wav`.
use visualizador_audio::fuentes::*;
use visualizador_audio::visual::Visualizador;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut fuente: Box<dyn Fuente> = match args.get(1) {
        Some(ruta) => {
            let (sr, m) = leer_wav(&std::fs::read(ruta).expect("no se pudo leer")).expect("WAV inválido");
            Box::new(Memoria::new(sr, m))
        }
        None => Box::new(Chirp::new(44_100, 100.0, 8_000.0, 2.0)),
    };
    let mut v = Visualizador::new(fuente.frecuencia_muestreo(), 2048, 32);
    let mut buf = vec![0.0; 2048];
    let mut cuadro = 0;
    while fuente.leer(&mut buf) == buf.len() {
        v.procesar(&buf);
        if cuadro % 14 == 0 {
            let f = v.centros()[v.banda_dominante()];
            println!("cuadro {cuadro}: dominante ≈ {f:.0} Hz\n{}", v.render_texto(8));
        }
        cuadro += 1;
    }
    std::fs::write("/tmp/espectro.ppm", v.render_ppm(8, 120)).ok();
    println!("Último cuadro guardado en /tmp/espectro.ppm");
}
