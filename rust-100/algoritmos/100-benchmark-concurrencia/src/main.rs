//! Benchmark: `cargo run --release [-- --limite 5000000 --tareas 256 --peticiones 10000 --ms 20]`
//!
//! Cada medición CPU es la **mediana de 3** ejecuciones. Además se imprime
//! el *speedup* respecto a la versión secuencial.
use benchmark_concurrencia::cpu::{self, Estrategia};
use benchmark_concurrencia::io::{self, EstrategiaIo};
use benchmark_concurrencia::{medir, nucleos};
use std::time::Duration;

fn arg(nombre: &str, def: u64) -> u64 {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == nombre).and_then(|i| a.get(i + 1)?.parse().ok()).unwrap_or(def)
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn main() {
    let hilos = nucleos();
    let limite = arg("--limite", 3_000_000);
    let n_tareas = arg("--tareas", 64) as usize;
    let peticiones = arg("--peticiones", 2_000);
    let espera = arg("--ms", 20);
    println!("Núcleos: {hilos}\n");

    println!("── CPU: primos < {limite} en {n_tareas} tareas ──");
    let ts = cpu::tareas(limite, n_tareas);
    let mut base = None;
    for e in Estrategia::TODAS {
        let mut tiempos = Vec::new();
        let mut resultado = 0;
        for _ in 0..3 {
            let (r, t) = medir(|| cpu::ejecutar(e, &ts, hilos));
            resultado = r;
            tiempos.push(t);
        }
        tiempos.sort();
        let t = tiempos[1];
        let b = *base.get_or_insert(t);
        println!(
            "  {:<20} {:>9.1} ms  speedup {:>5.2}x  (primos: {resultado})",
            format!("{e:?}"),
            ms(t),
            b.as_secs_f64() / t.as_secs_f64()
        );
    }

    println!("\n── I/O: {peticiones} peticiones de {espera} ms ──");
    let estrategias = [
        EstrategiaIo::Tokio,
        EstrategiaIo::TokioLimitado(100),
        EstrategiaIo::HiloPorPeticion,
        EstrategiaIo::Rayon,
        EstrategiaIo::Secuencial,
    ];
    for e in estrategias {
        // Las estrategias que bloquean tardarían demasiado con muchas
        // peticiones: se estima su tiempo con una muestra y se avisa.
        let n = match e {
            EstrategiaIo::Secuencial => peticiones.min(50),
            EstrategiaIo::Rayon => peticiones.min(400),
            _ => peticiones,
        };
        let (_, t) = medir(|| io::ejecutar(e, n, espera, hilos));
        let estimado = t.as_secs_f64() * peticiones as f64 / n as f64;
        let nota = if n < peticiones { format!("  (medido con {n}, extrapolado)") } else { String::new() };
        println!("  {:<20} {:>9.1} ms{nota}", format!("{e:?}"), estimado * 1e3);
    }
    println!("\nIdeal I/O: ≈ {espera} ms (todas las esperas solapadas).");
}
