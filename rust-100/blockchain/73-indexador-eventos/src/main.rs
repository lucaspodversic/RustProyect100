//! Demo: un token y un marketplace emiten eventos; el indexador los procesa,
//! sobrevive a un reorg y responde consultas.
use indexador_eventos::cadena_mock::CadenaMock;
use indexador_eventos::indexador::{Filtro, Indexador};

fn main() {
    let (token, market, ana, beto) = ([0xAA; 20], [0xBB; 20], [1; 20], [2; 20]);
    let mut c = CadenaMock::nueva();
    c.transfer(token, [0; 20], ana, 1_000);
    c.minar();
    c.transfer(token, ana, beto, 300);
    c.listado(market, 1, ana, 50);
    c.minar();
    c.venta(market, 1, beto, 50);
    c.minar();
    let mut idx = Indexador::nuevo(vec![]);
    idx.sincronizar(&c, 0).unwrap();
    println!("Indexados {} eventos hasta el bloque {:?}", idx.registros.len(), idx.cursor);
    c.reorg(1);
    c.transfer(token, beto, ana, 10);
    c.minar();
    idx.sincronizar(&c, 0).unwrap();
    println!("Tras reorg: {} reorgs, saldos = {:?}", idx.reorgs, idx.saldos(&token).values().collect::<Vec<_>>());
    let p = idx.consultar(&Filtro { participante: Some(ana), ..Default::default() }, 0, 10);
    println!("Eventos de ana: {}", p.items.len());
}
