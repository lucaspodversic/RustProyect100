//! Tests sin TTY: navegación, búsqueda y contenido renderizado.

use explorador_tui::app::{App, Tecla, Vista};
use explorador_tui::datos::{CadenaSimulada, FuenteCadena, Resultado};
use explorador_tui::renderizar_texto;

fn app() -> App<CadenaSimulada> {
    App::nueva(CadenaSimulada::generar(50, 7))
}

#[test]
fn cadena_simulada_es_determinista_y_enlazada() {
    let a = CadenaSimulada::generar(20, 3);
    let b = CadenaSimulada::generar(20, 3);
    assert_eq!(a.bloques, b.bloques);
    for w in a.bloques.windows(2) {
        assert_eq!(w[1].hash_previo, w[0].hash);
    }
}

#[test]
fn navegacion_lista_con_limites() {
    let mut a = app();
    a.tecla(Tecla::Arriba);
    assert_eq!(a.seleccion, 0);
    assert_eq!(a.altura_seleccionada(), 49);
    for _ in 0..100 {
        a.tecla(Tecla::PagAbajo);
    }
    assert_eq!(a.altura_seleccionada(), 0);
}

#[test]
fn abrir_bloque_y_tx_y_volver() {
    let mut a = app();
    // Buscamos un bloque con transacciones.
    let h = (0..50).rev().find(|h| !a.fuente.bloque(*h).unwrap().txs.is_empty()).unwrap();
    a.seleccion = (49 - h) as usize;
    a.tecla(Tecla::Enter);
    assert_eq!(a.vista, Vista::DetalleBloque { altura: h, tx_sel: 0 });
    a.tecla(Tecla::Enter);
    assert_eq!(a.vista, Vista::DetalleTx { altura: h, indice: 0 });
    a.tecla(Tecla::Esc);
    a.tecla(Tecla::Esc);
    assert_eq!(a.vista, Vista::Bloques);
}

#[test]
fn navegar_entre_bloques_con_h_l() {
    let mut a = app();
    a.ejecutar_busqueda("10");
    a.tecla(Tecla::Car('l'));
    assert_eq!(a.vista, Vista::DetalleBloque { altura: 11, tx_sel: 0 });
    a.tecla(Tecla::Car('h'));
    a.tecla(Tecla::Car('h'));
    assert_eq!(a.vista, Vista::DetalleBloque { altura: 9, tx_sel: 0 });
}

#[test]
fn busqueda_por_hash_de_bloque_y_tx() {
    let mut a = app();
    let hb = a.fuente.bloque(20).unwrap().hash[..10].to_string();
    a.tecla(Tecla::Car('/'));
    for c in hb.chars() {
        a.tecla(Tecla::Car(c));
    }
    a.tecla(Tecla::Enter);
    assert_eq!(a.vista, Vista::DetalleBloque { altura: 20, tx_sel: 0 });
    let (h, tx) = a.fuente.bloques.iter().find_map(|b| b.txs.first().map(|t| (b.altura, t.hash.clone()))).unwrap();
    assert_eq!(a.fuente.buscar_hash(&tx[..12]), Some(Resultado::Tx { altura: h, indice: 0 }));
}

#[test]
fn busqueda_sin_resultados_y_cancelada() {
    let mut a = app();
    a.ejecutar_busqueda("zzzzzz");
    assert!(a.estado.contains("Sin resultados"));
    a.tecla(Tecla::Car('/'));
    a.tecla(Tecla::Car('x'));
    a.tecla(Tecla::Borrar);
    a.tecla(Tecla::Esc);
    assert!(a.busqueda.is_none());
    a.ejecutar_busqueda("9999");
    assert!(a.estado.contains("No existe"));
}

#[test]
fn vista_direccion_desde_tx() {
    let mut a = app();
    let (h, de) = a.fuente.bloques.iter().find_map(|b| b.txs.first().map(|t| (b.altura, t.de.clone()))).unwrap();
    a.vista = Vista::DetalleTx { altura: h, indice: 0 };
    a.tecla(Tecla::Car('a'));
    assert_eq!(a.vista, Vista::Direccion(de.clone()));
    assert!(!a.fuente.txs_de_direccion(&de).is_empty());
    let txt = renderizar_texto(&a, 100, 20);
    assert!(txt.contains(&format!("Dirección {de}")));
}

#[test]
fn render_lista_muestra_altura_y_bloques() {
    let a = app();
    let txt = renderizar_texto(&a, 100, 20);
    assert!(txt.contains("Explorador"));
    assert!(txt.contains("altura: 49"));
    assert!(txt.contains("#49"));
    assert!(txt.contains("▶"));
}

#[test]
fn render_detalle_muestra_hash_completo() {
    let mut a = app();
    a.ejecutar_busqueda("5");
    let txt = renderizar_texto(&a, 120, 25);
    assert!(txt.contains(&a.fuente.bloque(5).unwrap().hash));
    assert!(txt.contains("Transacciones"));
}

#[test]
fn q_sale() {
    let mut a = app();
    a.tecla(Tecla::Car('q'));
    assert!(a.salir);
}
