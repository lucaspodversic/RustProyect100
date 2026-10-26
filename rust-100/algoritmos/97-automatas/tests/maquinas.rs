//! Pruebas de las máquinas de Turing de `maquinas/` y de la equivalencia AFN ≡ AFD.

use automatas::finito::{Afd, Afn};
use automatas::turing::{ejemplos, Maquina, Resultado};

#[test]
fn incremento_binario_coincide_con_aritmetica() {
    let m = Maquina::parsear(ejemplos::INCREMENTO_BINARIO).unwrap();
    for n in 0u32..300 {
        let e = m.ejecutar(&format!("{n:b}"), 10_000, None);
        assert_eq!(e.resultado, Resultado::Acepta);
        assert_eq!(e.cinta.contenido(), format!("{:b}", n + 1), "n={n}");
    }
}

#[test]
fn palindromos_exhaustivo_hasta_longitud_8() {
    let m = Maquina::parsear(ejemplos::PALINDROMO).unwrap();
    for len in 0..=8 {
        for bits in 0u32..(1 << len) {
            let w: String = (0..len).map(|i| if bits >> i & 1 == 1 { 'b' } else { 'a' }).collect();
            let es = w.chars().eq(w.chars().rev());
            let r = m.ejecutar(&w, 100_000, None).resultado;
            assert_eq!(r, if es { Resultado::Acepta } else { Resultado::Rechaza }, "{w}");
        }
    }
}

#[test]
fn castor_afanoso_3() {
    let m = Maquina::parsear(ejemplos::CASTOR_3).unwrap();
    let e = m.ejecutar("", 1_000, None);
    assert_eq!(e.resultado, Resultado::Acepta);
    assert_eq!(e.cinta.no_blancos(), 6, "Σ(3) = 6 unos");
    assert_eq!(e.pasos, 14);
}

#[test]
fn traza_llama_en_cada_paso() {
    let m = Maquina::parsear(ejemplos::INCREMENTO_BINARIO).unwrap();
    let mut n = 0;
    let mut f = |_: u64, _: &str, _: &automatas::turing::Cinta| n += 1;
    let e = m.ejecutar("11", 100, Some(&mut f));
    assert_eq!(n, e.pasos + 1); // estado inicial + uno por transición
}

#[test]
fn afn_y_su_afd_aceptan_lo_mismo() {
    let afn = Afn::kesimo_desde_final(3);
    let afd = afn.a_afd();
    let min = afd.minimizar();
    for len in 0..=10 {
        for bits in 0u32..(1 << len) {
            let w: String = (0..len).map(|i| if bits >> i & 1 == 1 { 'a' } else { 'b' }).collect();
            let esperado = w.len() >= 3 && w.as_bytes()[w.len() - 3] == b'a';
            assert_eq!(afn.acepta(&w), esperado, "{w}");
            assert_eq!(afd.acepta(&w), esperado);
            assert_eq!(min.acepta(&w), esperado);
        }
    }
}

#[test]
fn minimizar_multiplos() {
    // Curiosidad: el AFD "resto módulo k" NO siempre es mínimo. Para k = 6
    // los restos r y r+3 solo difieren en la paridad, que el siguiente bit
    // "olvida", así que bastan 4 estados. Para potencias de 2 (k = 2^j)
    // basta mirar los j últimos bits: j+1 estados. Para k impar sí es mínimo.
    for (k, esperado) in [(3, 3), (5, 5), (6, 4), (7, 7), (8, 4), (12, 5)] {
        let d = Afd::multiplo_de(k);
        let m = d.minimizar();
        assert_eq!(m.n, esperado, "k={k}");
        for n in 0u32..500 {
            assert_eq!(m.acepta(&format!("{n:b}")), n % k as u32 == 0);
        }
    }
}
