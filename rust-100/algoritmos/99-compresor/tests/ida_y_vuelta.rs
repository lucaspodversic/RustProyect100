//! Propiedades: descomprimir(comprimir(x)) == x para todo tipo de entradas,
//! y los datos corruptos producen error (nunca panic).

use compresor::contenedor::{comprimir, comprimir_con, descomprimir, info, Metodo};
use compresor::Error;

fn rng(mut x: u64) -> impl FnMut() -> u64 {
    move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    }
}

fn corpus() -> Vec<Vec<u8>> {
    let mut r = rng(99);
    let texto = "En un lugar de la Mancha, de cuyo nombre no quiero acordarme, no ha mucho tiempo \
                 que vivía un hidalgo de los de lanza en astillero, adarga antigua, rocín flaco y galgo corredor. "
        .repeat(40)
        .into_bytes();
    vec![
        vec![],
        vec![42],
        vec![0; 10_000],
        (0..=255u8).collect(),
        (0..=255u8).cycle().take(5000).collect(),
        (0..20_000).map(|_| r() as u8).collect(),
        (0..20_000).map(|_| if r() % 10 == 0 { r() as u8 } else { 0 }).collect(),
        texto,
    ]
}

#[test]
fn todos_los_metodos_ida_y_vuelta() {
    for (i, d) in corpus().iter().enumerate() {
        for m in Metodo::TODOS {
            let c = comprimir_con(d, m);
            assert_eq!(descomprimir(&c).as_ref(), Ok(d), "entrada {i} método {m:?}");
        }
        assert_eq!(&descomprimir(&comprimir(d)).unwrap(), d);
    }
}

#[test]
fn el_texto_comprime_y_lo_aleatorio_no_crece() {
    let c = corpus();
    let texto = &c[7];
    let comp = comprimir(texto);
    assert!(comp.len() * 100 < texto.len() * 60, "ratio {}/{}", comp.len(), texto.len());
    let aleatorio = &c[5];
    let comp = comprimir(aleatorio);
    assert_eq!(info(&comp).unwrap().0, Metodo::Almacenado);
    assert_eq!(comp.len(), aleatorio.len() + 17);
    let ceros = comprimir(&c[2]);
    assert!(ceros.len() < 400, "10 000 ceros → {} bytes", ceros.len());
}

#[test]
fn corrupcion_detectada_sin_panic() {
    let texto = corpus().pop().unwrap();
    for m in Metodo::TODOS {
        let bueno = comprimir_con(&texto, m);
        // Cambiar un bit en cada posición (muestreada) de la carga útil.
        for pos in (17..bueno.len()).step_by(97) {
            let mut malo = bueno.clone();
            malo[pos] ^= 0x10;
            assert!(descomprimir(&malo).is_err(), "{m:?} pos {pos} no detectado");
        }
        // Truncar.
        assert!(descomprimir(&bueno[..bueno.len() / 2]).is_err());
    }
    assert_eq!(descomprimir(b"XXXX\0").unwrap_err(), Error::Truncado);
    assert!(matches!(descomprimir(b"ZIP!00000000000000000"), Err(Error::Formato(_))));
}

#[test]
fn basura_aleatoria_no_provoca_panic() {
    let mut r = rng(5);
    for _ in 0..2000 {
        let len = (r() % 400) as usize;
        let mut d: Vec<u8> = (0..len).map(|_| r() as u8).collect();
        if d.len() > 17 {
            d[..4].copy_from_slice(b"CMP1");
            d[4] = (r() % 4) as u8;
            d[5..13].copy_from_slice(&((r() % 5000)).to_le_bytes());
        }
        let _ = descomprimir(&d); // solo importa que no haga panic
    }
}
