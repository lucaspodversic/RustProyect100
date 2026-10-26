//! Tests con **vectores oficiales** (EIP-155, EIP-55, Bitcoin) + propiedades.

use billetera::claves::*;
use billetera::rlp::{codificar, decodificar, Item};
use billetera::tx::*;

#[test]
fn eip155_vector_oficial() {
    // Ejemplo exacto de la especificación EIP-155.
    let k = ParClaves::desde_hex("0x4646464646464646464646464646464646464646464646464646464646464646").unwrap();
    let tx = Transaccion { nonce: 9, gas_price: 20_000_000_000, gas_limit: 21_000, to: Some([0x35; 20]), value: 1_000_000_000_000_000_000, data: vec![], chain_id: 1 };
    assert_eq!(hex::encode(tx.hash_firma()), "daf5a779ae972f972197303d7b574746c7ef83eadac0f2791ad23db92e4c8e53");
    let f = tx.firmar(&k);
    assert_eq!(f.v, 37);
    assert_eq!(hex::encode(f.rlp()), "f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83");
    assert_eq!(f.recuperar_remitente().unwrap(), k.direccion_eth_bytes());
}

#[test]
fn direccion_eth_conocida() {
    let k = ParClaves::desde_hex("4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318").unwrap();
    assert_eq!(k.direccion_eth(), "0x2c7536E3605D9C16a7a3D7b1898e529396a65c23");
}

#[test]
fn eip55_vectores_de_la_especificacion() {
    for d in ["0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed", "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359", "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB", "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb"] {
        let b = validar_direccion_eth(d).unwrap();
        assert_eq!(checksum_eip55(&b), d);
    }
    // Un typo de mayúscula invalida el checksum.
    assert!(validar_direccion_eth("0x5aaeb6053F3E94C9b9A09f33669435E7Ef1BeAed").is_err());
    // Todo minúsculas es válido (sin checksum).
    assert!(validar_direccion_eth("0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed").is_ok());
    assert!(validar_direccion_eth("5aaeb6").is_err());
}

#[test]
fn direccion_bitcoin_clave_1() {
    let mut b = [0u8; 32];
    b[31] = 1;
    let k = ParClaves::desde_bytes(&b).unwrap();
    assert_eq!(k.direccion_btc(), "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH");
}

#[test]
fn claves_invalidas() {
    assert_eq!(ParClaves::desde_bytes(&[0u8; 32]).unwrap_err(), ErrorClave::ClaveInvalida);
    assert_eq!(ParClaves::desde_bytes(&[1u8; 31]).unwrap_err(), ErrorClave::ClaveInvalida);
    assert_eq!(ParClaves::desde_hex("zz").unwrap_err(), ErrorClave::Hex);
    // n (orden de la curva) no es una clave válida.
    assert!(ParClaves::desde_hex("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141").is_err());
}

#[test]
fn generar_exportar_importar() {
    let k = ParClaves::generar();
    let k2 = ParClaves::desde_hex(&k.exportar_hex()).unwrap();
    assert_eq!(k.direccion_eth(), k2.direccion_eth());
    assert_ne!(ParClaves::generar().direccion_eth(), k.direccion_eth());
    assert!(!format!("{k:?}").contains(&k.exportar_hex()), "Debug no debe filtrar la clave");
}

#[test]
fn rlp_vectores_basicos() {
    assert_eq!(codificar(&Item::Bytes(b"dog".to_vec())), vec![0x83, b'd', b'o', b'g']);
    assert_eq!(codificar(&Item::Lista(vec![Item::Bytes(b"cat".to_vec()), Item::Bytes(b"dog".to_vec())])), vec![0xc8, 0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g']);
    assert_eq!(codificar(&Item::Bytes(vec![])), vec![0x80]);
    assert_eq!(codificar(&Item::entero(0)), vec![0x80]);
    assert_eq!(codificar(&Item::entero(15)), vec![0x0f]);
    assert_eq!(codificar(&Item::entero(1024)), vec![0x82, 0x04, 0x00]);
    assert_eq!(codificar(&Item::Lista(vec![])), vec![0xc0]);
    let largo = Item::Bytes(vec![b'a'; 56]);
    let c = codificar(&largo);
    assert_eq!(&c[..2], &[0xb8, 56]);
    assert_eq!(decodificar(&c).unwrap(), (largo, 58));
}

#[test]
fn roundtrip_raw_y_creacion_de_contrato() {
    let k = ParClaves::generar();
    let tx = Transaccion { nonce: 0, gas_price: 1, gas_limit: 100_000, to: None, value: 0, data: vec![0x60; 100], chain_id: 11155111 };
    let f = tx.firmar(&k);
    let p = TxFirmada::desde_rlp(&f.rlp()).unwrap();
    assert_eq!(p, f);
    assert_eq!(p.recuperar_remitente().unwrap(), k.direccion_eth_bytes());
}

#[test]
fn manipular_tx_cambia_el_remitente_recuperado() {
    let k = ParClaves::generar();
    let tx = Transaccion { nonce: 1, gas_price: 1, gas_limit: 21_000, to: Some([1; 20]), value: 5, data: vec![], chain_id: 1 };
    let mut f = tx.firmar(&k);
    f.tx.value = 5_000;
    // ecrecover devuelve otra dirección (o falla): nunca la del firmante real.
    assert_ne!(f.recuperar_remitente().ok(), Some(k.direccion_eth_bytes()));
}

#[test]
fn replay_en_otra_cadena_no_valida() {
    let k = ParClaves::generar();
    let tx = Transaccion { nonce: 1, gas_price: 1, gas_limit: 21_000, to: Some([1; 20]), value: 5, data: vec![], chain_id: 1 };
    let mut f = tx.firmar(&k);
    f.tx.chain_id = 137; // intentan reutilizar la firma en Polygon
    assert_ne!(f.recuperar_remitente().ok(), Some(k.direccion_eth_bytes()));
}

#[test]
fn firma_de_mensajes_eip191() {
    let k = ParClaves::generar();
    let s = firmar_mensaje(&k, b"hola mundo");
    assert_eq!(recuperar_firmante_mensaje(b"hola mundo", &s), Some(k.direccion_eth_bytes()));
    assert_ne!(recuperar_firmante_mensaje(b"chau mundo", &s), Some(k.direccion_eth_bytes()));
}

#[test]
fn rlp_basura_no_paniquea() {
    for d in [&[][..], &[0xf8], &[0xc5, 0x01], &[0xb9, 0xff, 0xff], &[0xc0]] {
        assert!(TxFirmada::desde_rlp(d).is_err());
    }
}
