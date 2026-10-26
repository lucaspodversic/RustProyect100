//! CLI de ejemplo.
//!
//! ```text
//! automatas mt <archivo.tm> <entrada> [--traza] [--max N]
//! automatas mt incremento|palindromo|castor3 <entrada> [--traza]
//! automatas multiplo <k> <binario>
//! automatas explosion <k>
//! ```
use automatas::finito::{Afd, Afn};
use automatas::turing::{ejemplos, Maquina, Resultado};
use std::process::ExitCode;

fn uso() -> ExitCode {
    eprintln!(
        "uso:\n  automatas mt <archivo.tm|incremento|palindromo|castor3> [entrada] [--traza] [--max N]\n  \
         automatas multiplo <k> <binario>\n  automatas explosion <k>"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let traza = args.iter().any(|a| a == "--traza");
    let max = args
        .iter()
        .position(|a| a == "--max")
        .and_then(|i| args.get(i + 1)?.parse().ok())
        .unwrap_or(1_000_000u64);
    // Argumentos posicionales (sin las opciones).
    let pos: Vec<&str> = {
        let mut v = Vec::new();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            if a == "--max" {
                it.next();
            } else if !a.starts_with("--") {
                v.push(a.as_str());
            }
        }
        v
    };
    match pos.as_slice() {
        ["mt", fuente, resto @ ..] => {
            let texto = match *fuente {
                "incremento" => ejemplos::INCREMENTO_BINARIO.to_string(),
                "palindromo" => ejemplos::PALINDROMO.to_string(),
                "castor3" => ejemplos::CASTOR_3.to_string(),
                ruta => match std::fs::read_to_string(ruta) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("no se pudo leer {ruta}: {e}");
                        return ExitCode::FAILURE;
                    }
                },
            };
            let m = match Maquina::parsear(&texto) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("error de formato: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let entrada = resto.first().copied().unwrap_or("");
            let mut imprimir = |p: u64, q: &str, c: &automatas::turing::Cinta| println!("{p:>5}  {q:<8} {c}");
            let e = m.ejecutar(entrada, max, if traza { Some(&mut imprimir) } else { None });
            println!("resultado: {:?}  pasos: {}  cinta: {:?}", e.resultado, e.pasos, e.cinta.contenido());
            if e.resultado == Resultado::Acepta { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        ["multiplo", k, w] => {
            let Ok(k) = k.parse::<usize>() else { return uso() };
            if k == 0 {
                return uso();
            }
            let si = Afd::multiplo_de(k).acepta(w);
            println!("{w} {} múltiplo de {k}", if si { "ES" } else { "NO es" });
            ExitCode::SUCCESS
        }
        ["explosion", k] => {
            let Ok(k) = k.parse::<usize>() else { return uso() };
            let afn = Afn::kesimo_desde_final(k.clamp(1, 16));
            let afd = afn.a_afd();
            println!("AFN: {} estados → AFD (subconjuntos): {} → mínimo: {}", afn.n, afd.n, afd.minimizar().n);
            ExitCode::SUCCESS
        }
        _ => uso(),
    }
}
