//! Codificación RLP (Recursive Length Prefix), el formato de serialización de
//! Ethereum para transacciones.
//!
//! Reglas:
//! - Un byte < 0x80 se codifica como sí mismo.
//! - Cadena de 0–55 bytes: `0x80 + len` seguido de los bytes.
//! - Cadena más larga: `0xb7 + len(len)` + len big-endian + bytes.
//! - Lista: igual pero con bases `0xc0` / `0xf7` sobre la concatenación.
//! - Los enteros se codifican big-endian **sin ceros a la izquierda** (0 → cadena vacía).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Bytes(Vec<u8>),
    Lista(Vec<Item>),
}

impl Item {
    /// Entero sin signo → bytes mínimos big-endian.
    pub fn entero(n: u128) -> Self {
        let b = n.to_be_bytes();
        let i = b.iter().position(|x| *x != 0).unwrap_or(b.len());
        Item::Bytes(b[i..].to_vec())
    }
}

fn prefijo(len: usize, base_corta: u8, base_larga: u8) -> Vec<u8> {
    if len <= 55 {
        vec![base_corta + len as u8]
    } else {
        let lb = (len as u64).to_be_bytes();
        let i = lb.iter().position(|x| *x != 0).unwrap();
        let mut v = vec![base_larga + (8 - i) as u8];
        v.extend_from_slice(&lb[i..]);
        v
    }
}

pub fn codificar(item: &Item) -> Vec<u8> {
    match item {
        Item::Bytes(b) if b.len() == 1 && b[0] < 0x80 => b.clone(),
        Item::Bytes(b) => {
            let mut v = prefijo(b.len(), 0x80, 0xb7);
            v.extend_from_slice(b);
            v
        }
        Item::Lista(xs) => {
            let cuerpo: Vec<u8> = xs.iter().flat_map(codificar).collect();
            let mut v = prefijo(cuerpo.len(), 0xc0, 0xf7);
            v.extend(cuerpo);
            v
        }
    }
}

/// Decodificador (para verificar transacciones recibidas). Devuelve el ítem y
/// los bytes consumidos.
pub fn decodificar(d: &[u8]) -> Option<(Item, usize)> {
    let p = *d.first()?;
    let largo = |n: usize| -> Option<usize> {
        let b = d.get(1..1 + n)?;
        Some(b.iter().fold(0usize, |acc, x| (acc << 8) | *x as usize))
    };
    match p {
        0x00..=0x7f => Some((Item::Bytes(vec![p]), 1)),
        0x80..=0xb7 => {
            let n = (p - 0x80) as usize;
            Some((Item::Bytes(d.get(1..1 + n)?.to_vec()), 1 + n))
        }
        0xb8..=0xbf => {
            let ll = (p - 0xb7) as usize;
            let n = largo(ll)?;
            Some((Item::Bytes(d.get(1 + ll..1 + ll + n)?.to_vec()), 1 + ll + n))
        }
        _ => {
            let (ini, n) = if p <= 0xf7 { (1, (p - 0xc0) as usize) } else {
                let ll = (p - 0xf7) as usize;
                (1 + ll, largo(ll)?)
            };
            let mut cuerpo = d.get(ini..ini + n)?;
            let mut xs = vec![];
            while !cuerpo.is_empty() {
                let (it, c) = decodificar(cuerpo)?;
                xs.push(it);
                cuerpo = &cuerpo[c..];
            }
            Some((Item::Lista(xs), ini + n))
        }
    }
}
