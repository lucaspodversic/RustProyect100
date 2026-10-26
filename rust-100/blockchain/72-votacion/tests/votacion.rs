use ed25519_dalek::{Signer, SigningKey};
use votacion_descentralizada::*;

const CAL: Calendario = Calendario { inicio_votacion: 10, fin_votacion: 20, fin_revelacion: 30 };

fn publico() -> ContratoVotacion {
    let mut c = ContratoVotacion::desplegar("presi", &["A", "B", "C"], Modalidad::Publica, CAL, 0);
    for v in ["ana", "beto", "caro", "dani"] {
        c.registrar("presi", v, 1, None, 0).unwrap();
    }
    c
}

#[test]
fn fases_por_altura() {
    let c = publico();
    assert_eq!(c.fase(0), Fase::Registro);
    assert_eq!(c.fase(10), Fase::Votacion);
    assert_eq!(c.fase(20), Fase::Cerrada);
    let cr = ContratoVotacion::desplegar("p", &["A"], Modalidad::CommitReveal, CAL, 0);
    assert_eq!(cr.fase(25), Fase::Revelacion);
    assert_eq!(cr.fase(30), Fase::Cerrada);
}

#[test]
fn solo_presidente_registra_y_solo_en_registro() {
    let mut c = publico();
    assert_eq!(c.registrar("ana", "x", 1, None, 0), Err(ErrorVoto::NoAutorizado));
    assert_eq!(c.registrar("presi", "x", 1, None, 10), Err(ErrorVoto::FaseIncorrecta(Fase::Registro)));
    assert_eq!(c.registrar("presi", "ana", 5, None, 0), Err(ErrorVoto::YaRegistrado));
}

#[test]
fn voto_simple_y_doble_voto() {
    let mut c = publico();
    c.votar("ana", 0, 10).unwrap();
    assert_eq!(c.votar("ana", 1, 11), Err(ErrorVoto::YaVoto));
    assert_eq!(c.votar("mallory", 0, 11), Err(ErrorVoto::SinDerecho));
    assert_eq!(c.votar("beto", 9, 11), Err(ErrorVoto::PropuestaInvalida));
    assert_eq!(c.votar("beto", 0, 5), Err(ErrorVoto::FaseIncorrecta(Fase::Votacion)));
    assert_eq!(c.propuestas[0].votos, 1);
}

#[test]
fn delegacion_transitiva_antes_y_despues_del_voto() {
    let mut c = publico();
    // beto → caro → dani ; dani vota B: suma 3.
    c.delegar("caro", "dani", 10).unwrap();
    c.delegar("beto", "caro", 10).unwrap(); // sigue la cadena hasta dani
    c.votar("dani", 1, 11).unwrap();
    assert_eq!(c.propuestas[1].votos, 3);
    // ana delega en dani que YA votó: suma directo a B.
    c.delegar("ana", "dani", 12).unwrap();
    assert_eq!(c.propuestas[1].votos, 4);
}

#[test]
fn delegacion_ciclica_y_autodelegacion() {
    let mut c = publico();
    assert_eq!(c.delegar("ana", "ana", 10), Err(ErrorVoto::AutoDelegacion));
    c.delegar("ana", "beto", 10).unwrap();
    c.delegar("beto", "caro", 10).unwrap();
    assert_eq!(c.delegar("caro", "ana", 10), Err(ErrorVoto::CicloDelegacion));
    assert_eq!(c.votar("ana", 0, 10), Err(ErrorVoto::YaVoto));
}

#[test]
fn quorum_y_ganadora_con_desempate() {
    let mut c = ContratoVotacion::desplegar("presi", &["A", "B"], Modalidad::Publica, CAL, 60);
    for v in ["ana", "beto", "caro", "dani"] {
        c.registrar("presi", v, 1, None, 0).unwrap();
    }
    // Peso total 5 (4 + presidente). Votan 2 → 40% < 60% → sin ganadora.
    c.votar("ana", 1, 10).unwrap();
    c.votar("beto", 0, 10).unwrap();
    assert_eq!(c.finalizar(15), Err(ErrorVoto::FaseIncorrecta(Fase::Cerrada)));
    assert_eq!(c.finalizar(20), Ok(None));

    let mut d = publico(); // quórum 0
    d.votar("ana", 1, 10).unwrap();
    d.votar("beto", 0, 10).unwrap();
    assert_eq!(d.finalizar(20), Ok(Some(0)), "empate → gana la de menor índice");
    assert_eq!(d.fase(0), Fase::Cerrada);
    assert_eq!(d.eventos.last(), Some(&Evento::Finalizada { ganadora: Some(0) }));
}

#[test]
fn commit_reveal_completo() {
    let mut c = ContratoVotacion::desplegar("presi", &["A", "B"], Modalidad::CommitReveal, CAL, 0);
    c.registrar("presi", "ana", 2, None, 0).unwrap();
    c.registrar("presi", "beto", 1, None, 0).unwrap();
    c.comprometer("ana", compromiso(1, b"sal-ana", "ana"), 10).unwrap();
    c.comprometer("beto", compromiso(0, b"sal-beto", "beto"), 11).unwrap();
    // Durante la votación nada se sabe.
    assert!(c.propuestas.iter().all(|p| p.votos == 0));
    assert_eq!(c.votar("ana", 1, 12), Err(ErrorVoto::ModalidadIncorrecta));
    // No se puede revelar antes de tiempo.
    assert_eq!(c.revelar("ana", 1, b"sal-ana", 15), Err(ErrorVoto::FaseIncorrecta(Fase::Revelacion)));
    // Revelación con datos falsos falla; la correcta suma.
    assert_eq!(c.revelar("ana", 0, b"sal-ana", 21), Err(ErrorVoto::RevelacionInvalida));
    c.revelar("ana", 1, b"sal-ana", 21).unwrap();
    assert_eq!(c.revelar("ana", 1, b"sal-ana", 22), Err(ErrorVoto::YaVoto));
    c.revelar("beto", 0, b"sal-beto", 22).unwrap();
    assert_eq!(c.finalizar(30), Ok(Some(1)));
}

#[test]
fn commit_copiado_no_sirve_a_otro_votante() {
    let mut c = ContratoVotacion::desplegar("presi", &["A", "B"], Modalidad::CommitReveal, CAL, 0);
    c.registrar("presi", "ana", 1, None, 0).unwrap();
    c.registrar("presi", "mallory", 1, None, 0).unwrap();
    let h = compromiso(1, b"s", "ana");
    c.comprometer("ana", h, 10).unwrap();
    c.comprometer("mallory", h, 10).unwrap(); // copia el hash de ana
    c.revelar("ana", 1, b"s", 20).unwrap();
    assert_eq!(c.revelar("mallory", 1, b"s", 20), Err(ErrorVoto::RevelacionInvalida));
}

#[test]
fn voto_firmado_ed25519_con_nonce() {
    let sk = SigningKey::generate(&mut rand::rngs::OsRng);
    let mut c = ContratoVotacion::desplegar("presi", &["A", "B"], Modalidad::Publica, CAL, 0);
    c.registrar("presi", "ana", 1, Some(sk.verifying_key().to_bytes()), 0).unwrap();
    let firma = sk.sign(&mensaje_voto("ctr-1", "ana", 1, 0)).to_bytes();
    // Firma para otro contrato o propuesta no sirve.
    assert_eq!(c.votar_firmado("ana", 0, &firma, "ctr-1", 10), Err(ErrorVoto::FirmaInvalida));
    assert_eq!(c.votar_firmado("ana", 1, &firma, "ctr-2", 10), Err(ErrorVoto::FirmaInvalida));
    c.votar_firmado("ana", 1, &firma, "ctr-1", 10).unwrap();
    assert_eq!(c.propuestas[1].votos, 1);
    // Replay: el nonce cambió, la misma firma ya no verifica.
    assert_eq!(c.votar_firmado("ana", 1, &firma, "ctr-1", 11), Err(ErrorVoto::FirmaInvalida));
}

#[test]
fn eventos_para_indexadores() {
    let mut c = publico();
    c.votar("ana", 2, 10).unwrap();
    assert!(c.eventos.contains(&Evento::VotanteRegistrado { votante: "ana".into(), peso: 1 }));
    assert!(c.eventos.contains(&Evento::Votado { votante: "ana".into(), propuesta: 2, peso: 1 }));
}

#[test]
fn presidente_puede_redefinir_su_peso_sin_contar_doble() {
    let mut c = ContratoVotacion::desplegar("presi", &["A"], Modalidad::Publica, CAL, 100);
    c.registrar("presi", "presi", 3, None, 0).unwrap();
    assert_eq!(c.peso_registrado, 3);
    c.votar("presi", 0, 10).unwrap();
    assert_eq!(c.finalizar(20), Ok(Some(0)), "100% de quórum alcanzado");
}
