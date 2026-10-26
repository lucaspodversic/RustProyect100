use indexador_eventos::cadena_mock::CadenaMock;
use indexador_eventos::indexador::{ErrorIndexador, Filtro, Indexador};
use indexador_eventos::modelo::*;

const TOKEN: Direccion = [0xAA; 20];
const MARKET: Direccion = [0xBB; 20];
const ANA: Direccion = [1; 20];
const BETO: Direccion = [2; 20];
const CERO: Direccion = [0; 20];

fn escenario() -> CadenaMock {
    let mut c = CadenaMock::nueva();
    c.transfer(TOKEN, CERO, ANA, 1_000); // mint
    c.minar();
    c.transfer(TOKEN, ANA, BETO, 250);
    c.listado(MARKET, 7, ANA, 99);
    c.minar();
    c.listado(MARKET, 8, BETO, 5);
    c.venta(MARKET, 7, BETO, 99);
    c.minar();
    c
}

#[test]
fn topic_transfer_es_el_estandar_erc20() {
    assert_eq!(hex::encode(topic_de(FIRMA_TRANSFER)), "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");
}

#[test]
fn decodifica_eventos() {
    let c = escenario();
    let l = &c.bloques[1].logs[0];
    assert_eq!(decodificar(l), Evento::Transfer { de: CERO, para: ANA, valor: 1_000 });
    assert_eq!(decodificar(&c.bloques[2].logs[1]), Evento::Listado { id: 7, vendedor: ANA, precio: 99 });
    let mut raro = l.clone();
    raro.topics = vec![[9; 32]];
    assert_eq!(decodificar(&raro), Evento::Desconocido);
}

#[test]
fn sincroniza_y_calcula_saldos() {
    let c = escenario();
    let mut i = Indexador::nuevo(vec![]);
    assert_eq!(i.sincronizar(&c, 0), Ok(4));
    assert_eq!(i.registros.len(), 5);
    let s = i.saldos(&TOKEN);
    assert_eq!(s[&ANA], 750);
    assert_eq!(s[&BETO], 250);
    assert_eq!(i.listados_activos(), vec![(8, BETO, 5)]);
}

#[test]
fn filtro_por_contrato_al_indexar() {
    let c = escenario();
    let mut i = Indexador::nuevo(vec![MARKET]);
    i.sincronizar(&c, 0).unwrap();
    assert_eq!(i.registros.len(), 3);
    assert!(i.saldos(&TOKEN).is_empty());
}

#[test]
fn confirmaciones_retrasan_la_ingesta() {
    let c = escenario();
    let mut i = Indexador::nuevo(vec![]);
    i.sincronizar(&c, 2).unwrap();
    assert_eq!(i.cursor, Some(1));
    assert_eq!(i.registros.len(), 1);
}

#[test]
fn consultas_con_filtros_y_paginacion() {
    let c = escenario();
    let mut i = Indexador::nuevo(vec![]);
    i.sincronizar(&c, 0).unwrap();
    let todos = i.consultar(&Filtro::default(), 0, 2);
    assert_eq!(todos.items.len(), 2);
    let p2 = i.consultar(&Filtro::default(), todos.siguiente.unwrap(), 2);
    let p3 = i.consultar(&Filtro::default(), p2.siguiente.unwrap(), 2);
    assert_eq!(p3.items.len(), 1);
    assert_eq!(p3.siguiente, None);
    let transfers = i.consultar(&Filtro { topic0: Some(topic_de(FIRMA_TRANSFER)), ..Default::default() }, 0, 100);
    assert_eq!(transfers.items.len(), 2);
    let rango = i.consultar(&Filtro { desde: Some(3), hasta: Some(3), ..Default::default() }, 0, 100);
    assert_eq!(rango.items.len(), 2);
    let de_beto = i.consultar(&Filtro { participante: Some(BETO), contrato: Some(MARKET), ..Default::default() }, 0, 100);
    assert_eq!(de_beto.items.len(), 2);
}

#[test]
fn reorg_revierte_eventos_huerfanos() {
    let mut c = escenario();
    let mut i = Indexador::nuevo(vec![]);
    i.sincronizar(&c, 0).unwrap();
    // Se descartan los bloques 2 y 3; la rama nueva tiene otra transferencia.
    c.reorg(2);
    c.transfer(TOKEN, ANA, BETO, 1);
    c.minar();
    c.minar();
    i.sincronizar(&c, 0).unwrap();
    assert!(i.reorgs >= 1);
    let s = i.saldos(&TOKEN);
    assert_eq!(s[&ANA], 999);
    assert_eq!(s[&BETO], 1);
    assert!(i.listados_activos().is_empty(), "los listados del bloque huérfano desaparecen");
    // Los índices secundarios también quedaron coherentes.
    let p = i.consultar(&Filtro { participante: Some(BETO), ..Default::default() }, 0, 100);
    assert_eq!(p.items.len(), 1);
}

#[test]
fn checkpoint_roundtrip_y_continuacion() {
    let mut c = escenario();
    let mut i = Indexador::nuevo(vec![]);
    i.sincronizar(&c, 0).unwrap();
    let json = i.guardar_checkpoint();
    let mut j = Indexador::cargar_checkpoint(&json).unwrap();
    assert_eq!(j.registros, i.registros);
    c.transfer(TOKEN, BETO, ANA, 50);
    c.minar();
    assert_eq!(j.sincronizar(&c, 0), Ok(1));
    assert_eq!(j.saldos(&TOKEN)[&BETO], 200);
    assert_eq!(j.consultar(&Filtro { participante: Some(ANA), ..Default::default() }, 0, 100).items.len(), 4);
    assert!(matches!(Indexador::cargar_checkpoint("{basura"), Err(ErrorIndexador::Checkpoint(_))));
}

#[test]
fn reorg_del_genesis_es_error() {
    let c = escenario();
    let mut i = Indexador::nuevo(vec![]);
    i.sincronizar(&c, 0).unwrap();
    let mut otra = CadenaMock::nueva();
    otra.rama = 9;
    otra.bloques.clear();
    otra.minar(); // génesis distinto
    assert_eq!(i.sincronizar(&otra, 0), Err(ErrorIndexador::ReorgProfundo));
}
