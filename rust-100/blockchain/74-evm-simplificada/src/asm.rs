//! Ensamblador mínimo para escribir programas legibles en los tests.
//!
//! Sintaxis (separada por espacios/saltos de línea, `;` comenta):
//! - `PUSH1 0x2a` / `PUSH2 300` — PUSHn con valor hex o decimal.
//! - `etiqueta:` — define una etiqueta (emite `JUMPDEST` automáticamente).
//! - `@etiqueta` — empuja la dirección de la etiqueta (como `PUSH2`).

use crate::opcodes::*;
use std::collections::HashMap;

pub fn ensamblar(src: &str) -> Result<Vec<u8>, String> {
    let tokens: Vec<&str> = src.lines().flat_map(|l| l.split(';').next().unwrap_or("").split_whitespace()).collect();
    let mut code = Vec::new();
    let mut etiquetas = HashMap::new();
    let mut parches: Vec<(usize, String)> = vec![];
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i];
        if let Some(nombre) = t.strip_suffix(':') {
            etiquetas.insert(nombre.to_string(), code.len());
            code.push(JUMPDEST);
        } else if let Some(nombre) = t.strip_prefix('@') {
            code.push(PUSH1 + 1); // PUSH2
            parches.push((code.len(), nombre.to_string()));
            code.extend([0, 0]);
        } else {
            let op = opcode_de(t).ok_or(format!("mnemónico desconocido: {t}"))?;
            code.push(op);
            let n = bytes_push(op);
            if n > 0 {
                i += 1;
                let arg = tokens.get(i).ok_or(format!("{t} sin argumento"))?;
                let bytes = parsear_num(arg).ok_or(format!("número inválido: {arg}"))?;
                if bytes.len() > n {
                    return Err(format!("{arg} no entra en {t}"));
                }
                code.extend(std::iter::repeat_n(0, n - bytes.len()));
                code.extend(bytes);
            }
        }
        i += 1;
    }
    for (pos, nombre) in parches {
        let d = *etiquetas.get(&nombre).ok_or(format!("etiqueta desconocida: {nombre}"))?;
        code[pos..pos + 2].copy_from_slice(&(d as u16).to_be_bytes());
    }
    Ok(code)
}

/// Número → bytes big-endian mínimos.
fn parsear_num(s: &str) -> Option<Vec<u8>> {
    let h = if let Some(h) = s.strip_prefix("0x") {
        h.to_string()
    } else {
        format!("{:x}", s.parse::<u128>().ok()?)
    };
    let h = if h.len() % 2 == 1 { format!("0{h}") } else { h };
    let b = hex::decode(h).ok()?;
    let ceros = b.iter().take_while(|x| **x == 0).count().min(b.len().saturating_sub(1));
    Some(b[ceros..].to_vec())
}
