//! Tipos de datos: bloques, logs y eventos decodificados.

use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

pub type Direccion = [u8; 20];
pub type H256 = [u8; 32];

/// Firma de evento → topic0 (keccak256 de la firma canónica), igual que en Solidity.
pub fn topic_de(firma: &str) -> H256 {
    Keccak256::digest(firma.as_bytes()).into()
}

/// Dirección → topic (alineada a la derecha en 32 bytes, como `indexed address`).
pub fn dir_a_topic(d: &Direccion) -> H256 {
    let mut t = [0u8; 32];
    t[12..].copy_from_slice(d);
    t
}

pub fn topic_a_dir(t: &H256) -> Direccion {
    t[12..].try_into().unwrap()
}

/// Log crudo tal como lo devuelve `eth_getLogs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Log {
    pub contrato: Direccion,
    pub topics: Vec<H256>,
    pub data: Vec<u8>,
    pub bloque: u64,
    pub hash_bloque: H256,
    pub indice_tx: u32,
    pub indice_log: u32,
}

/// Bloque con sus logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BloqueConLogs {
    pub numero: u64,
    pub hash: H256,
    pub padre: H256,
    pub logs: Vec<Log>,
}

/// Eventos de la DApp ya decodificados (token ERC-20 + marketplace).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Evento {
    Transfer { de: Direccion, para: Direccion, valor: u128 },
    Approval { dueno: Direccion, gastador: Direccion, valor: u128 },
    Listado { id: u64, vendedor: Direccion, precio: u128 },
    Venta { id: u64, comprador: Direccion, precio: u128 },
    Desconocido,
}

pub const FIRMA_TRANSFER: &str = "Transfer(address,address,uint256)";
pub const FIRMA_APPROVAL: &str = "Approval(address,address,uint256)";
pub const FIRMA_LISTADO: &str = "Listed(uint256,address,uint256)";
pub const FIRMA_VENTA: &str = "Sold(uint256,address,uint256)";

fn u256_a_u128(d: &[u8]) -> Option<u128> {
    // Los uint256 vienen en 32 bytes big-endian; aceptamos valores que entren en u128.
    if d.len() != 32 || d[..16].iter().any(|b| *b != 0) {
        return None;
    }
    Some(u128::from_be_bytes(d[16..].try_into().unwrap()))
}

pub fn u128_a_u256(v: u128) -> Vec<u8> {
    let mut o = vec![0u8; 16];
    o.extend(v.to_be_bytes());
    o
}

/// Decodifica un log según su topic0 (ABI simplificada).
pub fn decodificar(log: &Log) -> Evento {
    let Some(t0) = log.topics.first() else { return Evento::Desconocido };
    let t = &log.topics;
    let r = if *t0 == topic_de(FIRMA_TRANSFER) && t.len() == 3 {
        u256_a_u128(&log.data).map(|valor| Evento::Transfer { de: topic_a_dir(&t[1]), para: topic_a_dir(&t[2]), valor })
    } else if *t0 == topic_de(FIRMA_APPROVAL) && t.len() == 3 {
        u256_a_u128(&log.data).map(|valor| Evento::Approval { dueno: topic_a_dir(&t[1]), gastador: topic_a_dir(&t[2]), valor })
    } else if *t0 == topic_de(FIRMA_LISTADO) && t.len() == 3 {
        let id = u256_a_u128(&t[1]).map(|x| x as u64);
        id.zip(u256_a_u128(&log.data)).map(|(id, precio)| Evento::Listado { id, vendedor: topic_a_dir(&t[2]), precio })
    } else if *t0 == topic_de(FIRMA_VENTA) && t.len() == 3 {
        let id = u256_a_u128(&t[1]).map(|x| x as u64);
        id.zip(u256_a_u128(&log.data)).map(|(id, precio)| Evento::Venta { id, comprador: topic_a_dir(&t[2]), precio })
    } else {
        None
    };
    r.unwrap_or(Evento::Desconocido)
}
