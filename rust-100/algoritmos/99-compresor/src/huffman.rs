//! Huffman canónico.
//!
//! 1. Se cuentan las frecuencias de cada byte.
//! 2. Se construye el árbol de Huffman con un montículo de mínimos, uniendo
//!    siempre los dos nodos menos frecuentes. Del árbol solo se aprovecha la
//!    **profundidad** de cada hoja, es decir, la longitud de su código.
//! 3. Si alguna longitud supera [`MAX_BITS`] (pasa con frecuencias tipo
//!    Fibonacci), se reducen las frecuencias a la mitad y se repite. Es una
//!    heurística sencilla; el algoritmo óptimo es *package‑merge*.
//! 4. **Códigos canónicos**: a partir de las longitudes se asignan los códigos
//!    en orden (longitud, símbolo). Así el descompresor puede reconstruirlos
//!    con solo las 256 longitudes, sin necesidad de serializar el árbol.
//!    DEFLATE (zip/gzip) funciona igual.
//!
//! Formato de este módulo: `[longitud original: u64 LE][256 longitudes: u8][bits...]`.

use crate::bits::{EscritorBits, LectorBits};
use crate::Error;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Longitud máxima de un código.
pub const MAX_BITS: u8 = 15;

/// Calcula la longitud de código de cada símbolo (0 = no aparece).
pub fn longitudes(frec: &[u64; 256]) -> [u8; 256] {
    let mut f = *frec;
    loop {
        let lens = longitudes_sin_limite(&f);
        if lens.iter().all(|&l| l <= MAX_BITS) {
            return lens;
        }
        // Aplanar la distribución: dividir a la mitad, sin llegar a 0 si aparecía.
        for x in f.iter_mut().filter(|x| **x > 0) {
            *x = (*x >> 1) | 1;
        }
    }
}

fn longitudes_sin_limite(frec: &[u64; 256]) -> [u8; 256] {
    let mut lens = [0u8; 256];
    // Nodos: 0..256 son hojas; los internos se añaden detrás. padre[i] = índice del padre.
    let mut padre: Vec<usize> = vec![usize::MAX; 256];
    let mut monticulo: BinaryHeap<Reverse<(u64, usize)>> =
        (0..256).filter(|&s| frec[s] > 0).map(|s| Reverse((frec[s], s))).collect();
    match monticulo.len() {
        0 => return lens,
        1 => {
            // Un único símbolo: necesita al menos 1 bit para poder contarse.
            let Reverse((_, s)) = monticulo.pop().unwrap();
            lens[s] = 1;
            return lens;
        }
        _ => {}
    }
    while monticulo.len() > 1 {
        let Reverse((fa, a)) = monticulo.pop().unwrap();
        let Reverse((fb, b)) = monticulo.pop().unwrap();
        let nuevo = padre.len();
        padre.push(usize::MAX);
        padre[a] = nuevo;
        padre[b] = nuevo;
        monticulo.push(Reverse((fa + fb, nuevo)));
    }
    // Profundidad de cada hoja = número de saltos hasta la raíz. Como los
    // padres siempre tienen índice mayor, se podría calcular en una pasada
    // descendente; aquí basta con subir (≤ 255 saltos por hoja).
    for s in 0..256 {
        if frec[s] > 0 {
            let (mut n, mut d) = (s, 0u32);
            while padre[n] != usize::MAX {
                n = padre[n];
                d += 1;
            }
            lens[s] = d.min(255) as u8;
        }
    }
    lens
}

/// Asigna códigos canónicos a partir de las longitudes.
pub fn codigos_canonicos(lens: &[u8; 256]) -> [u32; 256] {
    let mut orden: Vec<usize> = (0..256).filter(|&s| lens[s] > 0).collect();
    orden.sort_by_key(|&s| (lens[s], s));
    let mut codigos = [0u32; 256];
    let (mut codigo, mut len_ant) = (0u32, 0u8);
    for s in orden {
        codigo <<= lens[s] - len_ant; // al pasar a códigos más largos, se añaden ceros
        codigos[s] = codigo;
        codigo += 1;
        len_ant = lens[s];
    }
    codigos
}

/// Comprime con Huffman canónico.
pub fn comprimir(datos: &[u8]) -> Vec<u8> {
    let mut frec = [0u64; 256];
    for &b in datos {
        frec[b as usize] += 1;
    }
    let lens = longitudes(&frec);
    let cods = codigos_canonicos(&lens);
    let mut w = EscritorBits::new();
    for &b in datos {
        w.escribir(cods[b as usize], lens[b as usize] as u32);
    }
    let mut sal = Vec::with_capacity(8 + 256 + datos.len() / 2);
    sal.extend_from_slice(&(datos.len() as u64).to_le_bytes());
    sal.extend_from_slice(&lens);
    sal.extend(w.terminar());
    sal
}

/// Descomprime. Decodificación canónica bit a bit: para cada longitud L se
/// conoce el primer código y cuántos hay, así que basta comparar rangos,
/// sin necesidad de reconstruir el árbol.
pub fn descomprimir(datos: &[u8]) -> Result<Vec<u8>, Error> {
    if datos.len() < 8 + 256 {
        return Err(Error::Truncado);
    }
    let n = u64::from_le_bytes(datos[..8].try_into().unwrap()) as usize;
    let lens: [u8; 256] = datos[8..264].try_into().unwrap();
    if lens.iter().any(|&l| l > MAX_BITS) {
        return Err(Error::Formato("longitud de código > 15"));
    }
    // Desigualdad de Kraft: Σ 2^-L ≤ 1, o el código no es prefijo.
    let kraft: u64 = lens.iter().filter(|&&l| l > 0).map(|&l| 1u64 << (MAX_BITS - l)).sum();
    if kraft > 1 << MAX_BITS {
        return Err(Error::Formato("longitudes que violan Kraft"));
    }
    if n > 0 && kraft == 0 {
        return Err(Error::Formato("sin símbolos"));
    }
    // Tablas canónicas.
    let mut cuenta = [0u32; MAX_BITS as usize + 1];
    for &l in lens.iter().filter(|&&l| l > 0) {
        cuenta[l as usize] += 1;
    }
    let mut orden: Vec<u8> = (0..=255u8).filter(|&s| lens[s as usize] > 0).collect();
    orden.sort_by_key(|&s| (lens[s as usize], s));
    let mut primer_codigo = [0u32; MAX_BITS as usize + 1];
    let mut primer_indice = [0u32; MAX_BITS as usize + 1];
    let (mut codigo, mut indice) = (0u32, 0u32);
    for l in 1..=MAX_BITS as usize {
        codigo <<= 1;
        primer_codigo[l] = codigo;
        primer_indice[l] = indice;
        codigo += cuenta[l];
        indice += cuenta[l];
    }
    // Reservamos con prudencia: `n` viene de la cabecera y podría ser enorme
    // en un archivo malicioso. Como cada símbolo ocupa ≥ 1 bit, n no puede
    // superar los bits disponibles.
    let bits_disponibles = (datos.len() - 264) * 8;
    if n > bits_disponibles {
        return Err(Error::Truncado);
    }
    let mut sal = Vec::with_capacity(n);
    let mut r = LectorBits::new(&datos[264..]);
    while sal.len() < n {
        let mut c = 0u32;
        let mut l = 0;
        loop {
            c = (c << 1) | r.bit()?;
            l += 1;
            if l > MAX_BITS as usize {
                return Err(Error::Formato("código inexistente"));
            }
            let desplaz = c.wrapping_sub(primer_codigo[l]);
            if c >= primer_codigo[l] && desplaz < cuenta[l] {
                sal.push(orden[(primer_indice[l] + desplaz) as usize]);
                break;
            }
        }
    }
    Ok(sal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ejemplo_de_libro() {
        // Frecuencias: a=45 b=13 c=12 d=16 e=9 f=5 (Cormen). Longitudes óptimas: a=1, b,c,d=3, e,f=4.
        let mut f = [0u64; 256];
        for (s, n) in [(b'a', 45), (b'b', 13), (b'c', 12), (b'd', 16), (b'e', 9), (b'f', 5)] {
            f[s as usize] = n;
        }
        let l = longitudes(&f);
        assert_eq!([l[97], l[98], l[99], l[100], l[101], l[102]], [1, 3, 3, 3, 4, 4]);
        let c = codigos_canonicos(&l);
        // Canónicos: a=0, b=100, c=101, d=110, e=1110, f=1111
        assert_eq!([c[97], c[98], c[99], c[100], c[101], c[102]], [0, 0b100, 0b101, 0b110, 0b1110, 0b1111]);
    }

    #[test]
    fn limite_de_longitud_con_fibonacci() {
        // Frecuencias de Fibonacci generan un árbol "en escalera" de
        // profundidad ≈ número de símbolos. Con 30 símbolos superaría 15.
        let mut f = [0u64; 256];
        let (mut a, mut b) = (1u64, 1u64);
        for s in 0..30 {
            f[s] = a;
            (a, b) = (b, a + b);
        }
        assert!(longitudes_sin_limite(&f).iter().any(|&l| l > MAX_BITS));
        let l = longitudes(&f);
        assert!(l.iter().all(|&x| x <= MAX_BITS));
        // Y sigue siendo un código prefijo válido (Kraft ≤ 1).
        let k: f64 = l.iter().filter(|&&x| x > 0).map(|&x| 0.5f64.powi(x as i32)).sum();
        assert!(k <= 1.0);
    }

    #[test]
    fn un_solo_simbolo_y_vacio() {
        for d in [&b""[..], b"zzzzzzzz"] {
            assert_eq!(descomprimir(&comprimir(d)).unwrap(), d);
        }
    }
}
