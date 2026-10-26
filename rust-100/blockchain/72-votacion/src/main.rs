//! Demo de una votación pública con delegación.
use votacion_descentralizada::*;

fn main() {
    let cal = Calendario { inicio_votacion: 10, fin_votacion: 20, fin_revelacion: 30 };
    let mut c = ContratoVotacion::desplegar("presi", &["Plaza", "Biblioteca", "Ciclovía"], Modalidad::Publica, cal, 50);
    for (v, p) in [("ana", 3), ("beto", 2), ("caro", 1), ("dani", 1)] {
        c.registrar("presi", v, p, None, 1).unwrap();
    }
    c.votar("ana", 1, 12).unwrap();
    c.delegar("beto", "ana", 13).unwrap();
    c.delegar("caro", "dani", 13).unwrap();
    c.votar("dani", 2, 14).unwrap();
    let g = c.finalizar(25).unwrap();
    for p in &c.propuestas {
        println!("{:<12} {} votos", p.nombre, p.votos);
    }
    println!("Ganadora: {:?}", g.map(|i| &c.propuestas[i].nombre));
    println!("{} eventos emitidos", c.eventos.len());
}
