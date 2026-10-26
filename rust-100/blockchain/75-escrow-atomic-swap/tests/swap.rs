use escrow_atomic_swap::cadena::*;
use escrow_atomic_swap::escrow::*;
use escrow_atomic_swap::swap::*;

fn preparar() -> (Cadena, Cadena, Acuerdo) {
    let a = Cadena::nueva("A", &[("alicia", 10), ("beto", 0)]);
    let b = Cadena::nueva("B", &[("alicia", 0), ("beto", 100)]);
    let ac = Acuerdo { alicia: "alicia".into(), beto: "beto".into(), monto_a: 2, monto_b: 30, t1: 48, t2: 24, margen: 6 };
    (a, b, ac)
}

fn avanzar(a: &mut Cadena, b: &mut Cadena, n: u64) {
    a.avanzar(n);
    b.avanzar(n);
}

#[test]
fn swap_feliz_ambos_cobran() {
    let (mut a, mut b, ac) = preparar();
    let al = Alicia::nueva();
    let ia = al.bloquear_en_a(&mut a, &ac).unwrap();
    let ib = beto_verificar_y_bloquear(&a, ia, &mut b, &ac).unwrap();
    assert_eq!(beto_reclamar_en_a(&mut a, ia, &b, ib, &ac), Err(ErrorSwap::SecretoNoRevelado));
    avanzar(&mut a, &mut b, 5);
    al.reclamar_en_b(&mut b, ib, &ac).unwrap();
    beto_reclamar_en_a(&mut a, ia, &b, ib, &ac).unwrap();
    assert_eq!((a.saldo("alicia"), a.saldo("beto")), (8, 2));
    assert_eq!((b.saldo("alicia"), b.saldo("beto")), (30, 70));
    assert_eq!((a.oferta_total(), b.oferta_total()), (10, 100));
}

#[test]
fn beto_nunca_bloquea_alicia_recupera_tras_t1() {
    let (mut a, mut b, ac) = preparar();
    let al = Alicia::nueva();
    let ia = al.bloquear_en_a(&mut a, &ac).unwrap();
    assert_eq!(a.reembolsar(ia), Err(ErrorCadena::NoVencido));
    avanzar(&mut a, &mut b, 48);
    a.reembolsar(ia).unwrap();
    assert_eq!(a.saldo("alicia"), 10);
}

#[test]
fn alicia_no_reclama_ambos_recuperan() {
    let (mut a, mut b, ac) = preparar();
    let al = Alicia::nueva();
    let ia = al.bloquear_en_a(&mut a, &ac).unwrap();
    let ib = beto_verificar_y_bloquear(&a, ia, &mut b, &ac).unwrap();
    avanzar(&mut a, &mut b, 24);
    b.reembolsar(ib).unwrap(); // Beto primero (T2 vence antes)
    // Alicia ya no puede reclamar en B.
    assert_eq!(al.reclamar_en_b(&mut b, ib, &ac), Err(ErrorSwap::Cadena(ErrorCadena::YaResuelto)));
    avanzar(&mut a, &mut b, 24);
    a.reembolsar(ia).unwrap();
    assert_eq!((a.saldo("alicia"), b.saldo("beto")), (10, 100));
}

#[test]
fn reclamo_tardio_es_rechazado() {
    let (mut a, mut b, ac) = preparar();
    let al = Alicia::nueva();
    let ia = al.bloquear_en_a(&mut a, &ac).unwrap();
    let ib = beto_verificar_y_bloquear(&a, ia, &mut b, &ac).unwrap();
    avanzar(&mut a, &mut b, 24);
    assert_eq!(al.reclamar_en_b(&mut b, ib, &ac), Err(ErrorSwap::Cadena(ErrorCadena::Vencido)));
}

#[test]
fn preimagen_incorrecta_receptor_incorrecto_y_doble_reclamo() {
    let mut c = Cadena::nueva("X", &[("a", 5)]);
    let id = c.bloquear("a", "b", 5, sha256(b"secreto"), 10).unwrap();
    assert_eq!(c.reclamar(id, "b", b"otro"), Err(ErrorCadena::PreimagenIncorrecta));
    assert_eq!(c.reclamar(id, "mallory", b"secreto"), Err(ErrorCadena::NoAutorizado));
    c.reclamar(id, "b", b"secreto").unwrap();
    assert_eq!(c.reclamar(id, "b", b"secreto"), Err(ErrorCadena::YaResuelto));
    c.avanzar(20);
    assert_eq!(c.reembolsar(id), Err(ErrorCadena::YaResuelto));
    assert_eq!(c.oferta_total(), 5);
}

#[test]
fn validaciones_al_bloquear() {
    let mut c = Cadena::nueva("X", &[("a", 5)]);
    assert_eq!(c.bloquear("a", "b", 6, [0; 32], 10), Err(ErrorCadena::SaldoInsuficiente));
    assert_eq!(c.bloquear("a", "b", 0, [0; 32], 10), Err(ErrorCadena::Invalido));
    assert_eq!(c.bloquear("a", "b", 1, [0; 32], 0), Err(ErrorCadena::Invalido));
    assert_eq!(c.bloquear("a", "a", 1, [0; 32], 10), Err(ErrorCadena::Invalido));
}

#[test]
fn beto_rechaza_htlc_tramposo() {
    let (mut a, mut b, ac) = preparar();
    let al = Alicia::nueva();
    // Monto menor al acordado.
    let ia = a.bloquear("alicia", "beto", 1, al.hashlock, 48).unwrap();
    assert_eq!(beto_verificar_y_bloquear(&a, ia, &mut b, &ac), Err(ErrorSwap::HtlcIncorrecto("monto")));
    // Timelock demasiado corto: Beto no tendría tiempo de reclamar en A.
    let ia2 = a.bloquear("alicia", "beto", 2, al.hashlock, 25).unwrap();
    assert_eq!(beto_verificar_y_bloquear(&a, ia2, &mut b, &ac), Err(ErrorSwap::MargenInsuficiente));
    // Receptor equivocado.
    let ia3 = a.bloquear("alicia", "carla", 2, al.hashlock, 48).unwrap();
    assert_eq!(beto_verificar_y_bloquear(&a, ia3, &mut b, &ac), Err(ErrorSwap::HtlcIncorrecto("receptor")));
    assert_eq!(b.saldo("beto"), 100, "Beto no arriesgó nada");
}

#[test]
fn escrow_dos_de_tres() {
    let mut e = Escrow::nuevo("comp", "vend", "arb", 50);
    assert_eq!(e.votar("comp", Decision::Devolver), Ok(None));
    assert_eq!(e.votar("vend", Decision::Liberar), Ok(None));
    assert_eq!(e.votar("mallory", Decision::Liberar), Err(ErrorEscrow::NoEsParte));
    // El árbitro desempata a favor del vendedor.
    assert_eq!(e.votar("arb", Decision::Liberar), Ok(Some(("vend".into(), 50))));
    assert_eq!(e.estado, EstadoEscrow::Liberado);
    assert_eq!(e.votar("comp", Decision::Liberar), Err(ErrorEscrow::Cerrado));

    let mut d = Escrow::nuevo("comp", "vend", "arb", 7);
    d.votar("comp", Decision::Devolver).unwrap();
    assert_eq!(d.votar("comp", Decision::Devolver), Ok(None), "votar dos veces no cuenta doble");
    assert_eq!(d.votar("vend", Decision::Devolver), Ok(Some(("comp".into(), 7))));
}
