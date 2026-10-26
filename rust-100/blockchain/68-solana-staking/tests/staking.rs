//! Tests del programa: flujo feliz + ataques típicos de Solana.

use solana_staking::cliente::Cliente;
use solana_staking::estado::{Pool, Posicion};
use solana_staking::programa::*;
use solana_staking::runtime::*;

fn k(n: &str) -> Pubkey {
    Pubkey::desde_nombre(n)
}

fn banco() -> Banco {
    let mut b = Banco::default();
    for n in ["aut", "ana", "beto", "mallory"] {
        b.airdrop(k(n), 1_000_000);
    }
    b
}

#[test]
fn instruccion_roundtrip() {
    for i in [Instruccion::InicializarPool { tasa_por_segundo: 7, bloqueo_segundos: -3 }, Instruccion::Depositar { monto: 9 }, Instruccion::Reclamar, Instruccion::Retirar { monto: 1 }, Instruccion::Fondear { monto: 2 }, Instruccion::Pausar { pausado: true }] {
        assert_eq!(Instruccion::desempaquetar(&i.empaquetar()).unwrap(), i);
    }
    assert_eq!(Instruccion::desempaquetar(&[99]), Err(ErrorPrograma::DatosInvalidos));
    assert_eq!(Instruccion::desempaquetar(&[1, 0]), Err(ErrorPrograma::DatosInvalidos));
}

#[test]
fn pda_es_determinista_y_distinta_por_semilla() {
    let (a, ba) = pda_posicion(&k("p"), &k("ana"));
    let (a2, ba2) = pda_posicion(&k("p"), &k("ana"));
    assert_eq!((a, ba), (a2, ba2));
    assert_ne!(a, pda_posicion(&k("p"), &k("beto")).0);
}

#[test]
fn deposito_y_estado() {
    let mut b = banco();
    let mut c = Cliente::inicializar(&mut b, k("aut"), 1_000, 10).unwrap();
    c.depositar(k("ana"), 400).unwrap();
    c.depositar(k("ana"), 100).unwrap();
    let pool = c.pool;
    assert_eq!(ver_posicion(&b, &pool, &k("ana")).unwrap().monto, 500);
    assert_eq!(ver_pool(&b, &pool).unwrap().total_stakeado, 500);
    assert_eq!(b.lamports(&k("ana")), 1_000_000 - 500);
    assert_eq!(b.lamports(&pda_boveda(&pool).0), 500);
}

#[test]
fn recompensas_proporcionales_al_tiempo_y_monto() {
    let mut b = banco();
    // tasa 1e6/1e9 = 0,1% por segundo
    let mut c = Cliente::inicializar(&mut b, k("aut"), 1_000_000, 0).unwrap();
    c.fondear(k("aut"), 100_000).unwrap();
    c.depositar(k("ana"), 10_000).unwrap();
    c.banco.reloj += 100; // 10_000 * 0,001 * 100 = 1_000
    c.reclamar(k("ana")).unwrap();
    assert_eq!(b.lamports(&k("ana")), 1_000_000 - 10_000 + 1_000);
}

#[test]
fn reclamo_limitado_por_tesoro() {
    let mut b = banco();
    let mut c = Cliente::inicializar(&mut b, k("aut"), 1_000_000, 0).unwrap();
    c.fondear(k("aut"), 300).unwrap();
    c.depositar(k("ana"), 10_000).unwrap();
    c.banco.reloj += 100;
    c.reclamar(k("ana")).unwrap();
    let pool = c.pool;
    assert_eq!(b.lamports(&pda_tesoro(&pool).0), 0);
    // Lo no pagado sigue pendiente (700 lamports en punto fijo).
    assert_eq!(ver_posicion(&b, &pool, &k("ana")).unwrap().pendiente, 700 * solana_staking::estado::ESCALA);
}

#[test]
fn bloqueo_impide_retiro_anticipado() {
    let mut b = banco();
    let mut c = Cliente::inicializar(&mut b, k("aut"), 0, 60).unwrap();
    c.depositar(k("ana"), 1_000).unwrap();
    c.banco.reloj += 59;
    assert_eq!(c.retirar(k("ana"), 1_000), Err(ErrorPrograma::Bloqueado));
    c.banco.reloj += 1;
    c.retirar(k("ana"), 1_000).unwrap();
    assert_eq!(b.lamports(&k("ana")), 1_000_000);
}

#[test]
fn no_se_retira_mas_de_lo_depositado() {
    let mut b = banco();
    let mut c = Cliente::inicializar(&mut b, k("aut"), 0, 0).unwrap();
    c.depositar(k("ana"), 100).unwrap();
    c.depositar(k("beto"), 100).unwrap();
    assert_eq!(c.retirar(k("ana"), 150), Err(ErrorPrograma::FondosInsuficientes));
}

#[test]
fn ataque_falta_de_firma() {
    let mut b = banco();
    let pool = Cliente::inicializar(&mut b, k("aut"), 0, 0).unwrap().pool;
    Cliente { banco: &mut b, pool }.depositar(k("ana"), 100).unwrap();
    // Mallory arma la tx con Ana como "usuario" pero sin su firma.
    let metas = [MetaCuenta::escribible(k("ana")), MetaCuenta::escribible(pool), MetaCuenta::escribible(pda_posicion(&pool, &k("ana")).0), MetaCuenta::escribible(pda_boveda(&pool).0)];
    assert_eq!(procesar(&mut b, &metas, &Instruccion::Retirar { monto: 100 }.empaquetar()), Err(ErrorPrograma::FaltaFirma));
}

#[test]
fn ataque_sustitucion_de_posicion() {
    let mut b = banco();
    let pool = Cliente::inicializar(&mut b, k("aut"), 0, 0).unwrap().pool;
    let mut c = Cliente { banco: &mut b, pool };
    c.depositar(k("ana"), 500).unwrap();
    c.depositar(k("mallory"), 1).unwrap();
    // Mallory firma, pero pasa la posición de Ana para retirar sus fondos.
    let metas = [MetaCuenta::firmante(k("mallory")), MetaCuenta::escribible(pool), MetaCuenta::escribible(pda_posicion(&pool, &k("ana")).0), MetaCuenta::escribible(pda_boveda(&pool).0)];
    assert_eq!(procesar(&mut b, &metas, &Instruccion::Retirar { monto: 500 }.empaquetar()), Err(ErrorPrograma::PdaIncorrecta));
}

#[test]
fn ataque_boveda_falsa() {
    let mut b = banco();
    let pool = Cliente::inicializar(&mut b, k("aut"), 0, 0).unwrap().pool;
    let metas = [MetaCuenta::firmante(k("ana")), MetaCuenta::escribible(pool), MetaCuenta::escribible(pda_posicion(&pool, &k("ana")).0), MetaCuenta::escribible(k("mallory"))];
    assert_eq!(procesar(&mut b, &metas, &Instruccion::Depositar { monto: 10 }.empaquetar()), Err(ErrorPrograma::PdaIncorrecta));
}

#[test]
fn ataque_type_cosplay_pool_falso() {
    let mut b = banco();
    let pool = Cliente::inicializar(&mut b, k("aut"), 0, 0).unwrap().pool;
    // Mallory crea una cuenta con datos de "Posicion" y la pasa como pool.
    let falsa = k("pool-falso");
    let pos = Posicion { dueno: k("mallory"), pool, monto: 0, ultimo_calculo: 0, desde: 0, pendiente: 0, bump: 0 };
    b.cuentas.insert(falsa, Cuenta { lamports: 0, datos: pos.serializar(), owner: id_programa() });
    let metas = [MetaCuenta::firmante(k("aut")), MetaCuenta::escribible(falsa)];
    assert_eq!(procesar(&mut b, &metas, &Instruccion::Pausar { pausado: true }.empaquetar()), Err(ErrorPrograma::NoInicializada));
    // Y una cuenta con datos de Pool pero dueño ajeno también se rechaza.
    let ajena = k("pool-ajeno");
    let p = Pool::deserializar(&b.cuentas[&pool].datos).unwrap();
    b.cuentas.insert(ajena, Cuenta { lamports: 0, datos: p.serializar(), owner: SYSTEM_PROGRAM });
    let metas = [MetaCuenta::firmante(k("aut")), MetaCuenta::escribible(ajena)];
    assert_eq!(procesar(&mut b, &metas, &Instruccion::Pausar { pausado: true }.empaquetar()), Err(ErrorPrograma::DuenoIncorrecto));
}

#[test]
fn solo_autoridad_pausa_y_pausa_bloquea_depositos() {
    let mut b = banco();
    let mut c = Cliente::inicializar(&mut b, k("aut"), 0, 0).unwrap();
    assert_eq!(c.pausar(k("mallory"), true), Err(ErrorPrograma::NoAutorizado));
    c.pausar(k("aut"), true).unwrap();
    assert_eq!(c.depositar(k("ana"), 1), Err(ErrorPrograma::Pausado));
    c.pausar(k("aut"), false).unwrap();
    c.depositar(k("ana"), 1).unwrap();
}

#[test]
fn doble_inicializacion_rechazada() {
    let mut b = banco();
    Cliente::inicializar(&mut b, k("aut"), 0, 0).unwrap();
    assert!(matches!(Cliente::inicializar(&mut b, k("aut"), 0, 0), Err(ErrorPrograma::YaInicializada)));
}

#[test]
fn atomicidad_ante_error() {
    let mut b = banco();
    let pool = Cliente::inicializar(&mut b, k("aut"), 0, 0).unwrap().pool;
    let antes = b.cuentas.clone();
    // Depósito mayor que el saldo: falla y no deja rastros.
    assert_eq!(Cliente { banco: &mut b, pool }.depositar(k("ana"), 2_000_000), Err(ErrorPrograma::FondosInsuficientes));
    assert_eq!(b.cuentas, antes);
}

#[test]
fn deposito_cero_rechazado() {
    let mut b = banco();
    let mut c = Cliente::inicializar(&mut b, k("aut"), 0, 0).unwrap();
    assert_eq!(c.depositar(k("ana"), 0), Err(ErrorPrograma::MontoCero));
}
