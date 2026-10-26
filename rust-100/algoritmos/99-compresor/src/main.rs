//! CLI:
//!
//! ```text
//! compresor c <entrada> <salida> [--metodo auto|almacenado|rle|huffman|rle+huffman]
//! compresor d <entrada> <salida>
//! compresor info <archivo.cmp>
//! compresor comparar <archivo>      # tabla con todos los métodos
//! ```
use compresor::contenedor::{comprimir, comprimir_con, descomprimir, info, Metodo};
use std::process::ExitCode;
use std::time::Instant;

fn leer(ruta: &str) -> Result<Vec<u8>, String> {
    std::fs::read(ruta).map_err(|e| format!("no se pudo leer {ruta}: {e}"))
}

fn ejecutar(args: &[String]) -> Result<(), String> {
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["c", ent, sal, resto @ ..] => {
            let datos = leer(ent)?;
            let metodo = match resto {
                [] | ["--metodo", "auto"] => None,
                ["--metodo", "almacenado"] => Some(Metodo::Almacenado),
                ["--metodo", "rle"] => Some(Metodo::Rle),
                ["--metodo", "huffman"] => Some(Metodo::Huffman),
                ["--metodo", "rle+huffman"] => Some(Metodo::RleHuffman),
                _ => return Err("método inválido".into()),
            };
            let t = Instant::now();
            let c = metodo.map_or_else(|| comprimir(&datos), |m| comprimir_con(&datos, m));
            std::fs::write(sal, &c).map_err(|e| e.to_string())?;
            println!(
                "{} → {} bytes ({:.1} %) con {:?} en {:.1} ms",
                datos.len(),
                c.len(),
                100.0 * c.len() as f64 / datos.len().max(1) as f64,
                info(&c).map_err(|e| e.to_string())?.0,
                t.elapsed().as_secs_f64() * 1e3
            );
            Ok(())
        }
        ["d", ent, sal] => {
            let d = descomprimir(&leer(ent)?).map_err(|e| e.to_string())?;
            std::fs::write(sal, &d).map_err(|e| e.to_string())?;
            println!("{} bytes restaurados (CRC verificado)", d.len());
            Ok(())
        }
        ["info", ent] => {
            let (m, n, crc) = info(&leer(ent)?).map_err(|e| e.to_string())?;
            println!("método: {m:?}\ntamaño original: {n} bytes\nCRC-32: {crc:08x}");
            Ok(())
        }
        ["comparar", ent] => {
            let datos = leer(ent)?;
            println!("{:<12} {:>10} {:>8} {:>10}", "método", "bytes", "%", "ms");
            for m in Metodo::TODOS {
                let t = Instant::now();
                let c = comprimir_con(&datos, m);
                let ms = t.elapsed().as_secs_f64() * 1e3;
                let pct = 100.0 * c.len() as f64 / datos.len().max(1) as f64;
                println!("{:<12} {:>10} {:>7.1}% {:>10.2}", format!("{m:?}"), c.len(), pct, ms);
            }
            Ok(())
        }
        _ => Err("uso: compresor c|d|info|comparar ... (ver README)".into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match ejecutar(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
