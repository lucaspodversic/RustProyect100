//! Transacciones Ethereum "legacy" firmadas con protección de repetición
//! EIP-155 (el `chain_id` entra en el hash firmado y en `v`).

use crate::claves::{direccion_eth_de, keccak256, ParClaves};
use crate::rlp::{codificar, decodificar, Item};
use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaccion {
    pub nonce: u64,
    pub gas_price: u128,
    pub gas_limit: u64,
    /// `None` = creación de contrato.
    pub to: Option<[u8; 20]>,
    pub value: u128,
    pub data: Vec<u8>,
    pub chain_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxFirmada {
    pub tx: Transaccion,
    pub v: u64,
    pub r: [u8; 32],
    pub s: [u8; 32],
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErrorTx {
    #[error("RLP mal formado")]
    Rlp,
    #[error("firma inválida")]
    Firma,
}

impl Transaccion {
    fn campos(&self) -> Vec<Item> {
        vec![
            Item::entero(self.nonce as u128),
            Item::entero(self.gas_price),
            Item::entero(self.gas_limit as u128),
            Item::Bytes(self.to.map(|t| t.to_vec()).unwrap_or_default()),
            Item::entero(self.value),
            Item::Bytes(self.data.clone()),
        ]
    }

    /// Hash a firmar: keccak(rlp([nonce, gasPrice, gas, to, value, data, chainId, 0, 0])).
    pub fn hash_firma(&self) -> [u8; 32] {
        let mut c = self.campos();
        c.extend([Item::entero(self.chain_id as u128), Item::entero(0), Item::entero(0)]);
        keccak256(&codificar(&Item::Lista(c)))
    }

    /// Firma con ECDSA determinista (RFC 6979). `v = recid + chain_id*2 + 35`.
    pub fn firmar(&self, claves: &ParClaves) -> TxFirmada {
        let h = self.hash_firma();
        let (sig, rec) = claves.firmante().sign_prehash_recoverable(&h).expect("firmar un hash de 32 bytes no falla");
        // k256 ya normaliza `s` a la mitad baja (EIP-2), y ajusta recid en consecuencia.
        let bytes = sig.to_bytes();
        TxFirmada { tx: self.clone(), v: rec.to_byte() as u64 + self.chain_id * 2 + 35, r: bytes[..32].try_into().unwrap(), s: bytes[32..].try_into().unwrap() }
    }
}

fn entero(it: &Item) -> Option<u128> {
    match it {
        Item::Bytes(b) if b.len() <= 16 => Some(b.iter().fold(0u128, |a, x| (a << 8) | *x as u128)),
        _ => None,
    }
}

fn a32(it: &Item) -> Option<[u8; 32]> {
    match it {
        Item::Bytes(b) if b.len() <= 32 => {
            let mut o = [0u8; 32];
            o[32 - b.len()..].copy_from_slice(b);
            Some(o)
        }
        _ => None,
    }
}

fn sin_ceros(b: &[u8; 32]) -> Vec<u8> {
    let i = b.iter().position(|x| *x != 0).unwrap_or(32);
    b[i..].to_vec()
}

impl TxFirmada {
    /// Serialización "raw" lista para `eth_sendRawTransaction`.
    pub fn rlp(&self) -> Vec<u8> {
        let mut c = self.tx.campos();
        c.extend([Item::entero(self.v as u128), Item::Bytes(sin_ceros(&self.r)), Item::Bytes(sin_ceros(&self.s))]);
        codificar(&Item::Lista(c))
    }

    pub fn hash(&self) -> [u8; 32] {
        keccak256(&self.rlp())
    }

    /// Parsea una tx raw.
    pub fn desde_rlp(d: &[u8]) -> Result<Self, ErrorTx> {
        let (Item::Lista(c), n) = decodificar(d).ok_or(ErrorTx::Rlp)? else { return Err(ErrorTx::Rlp) };
        if n != d.len() || c.len() != 9 {
            return Err(ErrorTx::Rlp);
        }
        let to = match &c[3] {
            Item::Bytes(b) if b.is_empty() => None,
            Item::Bytes(b) if b.len() == 20 => Some(b.as_slice().try_into().unwrap()),
            _ => return Err(ErrorTx::Rlp),
        };
        let data = match &c[5] {
            Item::Bytes(b) => b.clone(),
            _ => return Err(ErrorTx::Rlp),
        };
        let v = entero(&c[6]).ok_or(ErrorTx::Rlp)? as u64;
        if v < 35 {
            return Err(ErrorTx::Firma); // solo aceptamos EIP-155
        }
        let tx = Transaccion {
            nonce: entero(&c[0]).ok_or(ErrorTx::Rlp)? as u64,
            gas_price: entero(&c[1]).ok_or(ErrorTx::Rlp)?,
            gas_limit: entero(&c[2]).ok_or(ErrorTx::Rlp)? as u64,
            to,
            value: entero(&c[4]).ok_or(ErrorTx::Rlp)?,
            data,
            chain_id: (v - 35) / 2,
        };
        Ok(Self { tx, v, r: a32(&c[7]).ok_or(ErrorTx::Rlp)?, s: a32(&c[8]).ok_or(ErrorTx::Rlp)? })
    }

    /// Recupera la clave pública del firmante (ecrecover) y devuelve su dirección.
    pub fn recuperar_remitente(&self) -> Result<[u8; 20], ErrorTx> {
        let rec = RecoveryId::from_byte(((self.v - 35) % 2) as u8).ok_or(ErrorTx::Firma)?;
        let mut rs = [0u8; 64];
        rs[..32].copy_from_slice(&self.r);
        rs[32..].copy_from_slice(&self.s);
        let sig = Signature::from_slice(&rs).map_err(|_| ErrorTx::Firma)?;
        // Rechazamos `s` alto (maleabilidad, EIP-2).
        if sig.normalize_s().is_some() {
            return Err(ErrorTx::Firma);
        }
        let pk = VerifyingKey::recover_from_prehash(&self.tx.hash_firma(), &sig, rec).map_err(|_| ErrorTx::Firma)?;
        Ok(direccion_eth_de(&pk))
    }
}

/// Firma de mensajes "personal_sign" (EIP-191): evita que un mensaje firmado
/// pueda reinterpretarse como transacción.
pub fn firmar_mensaje(claves: &ParClaves, msg: &[u8]) -> [u8; 65] {
    let h = hash_mensaje(msg);
    let (sig, rec) = claves.firmante().sign_prehash_recoverable(&h).expect("hash de 32 bytes");
    let mut o = [0u8; 65];
    o[..64].copy_from_slice(&sig.to_bytes());
    o[64] = 27 + rec.to_byte();
    o
}

pub fn hash_mensaje(msg: &[u8]) -> [u8; 32] {
    let mut d = format!("\x19Ethereum Signed Message:\n{}", msg.len()).into_bytes();
    d.extend_from_slice(msg);
    keccak256(&d)
}

/// Verifica una firma EIP-191 devolviendo la dirección del firmante.
pub fn recuperar_firmante_mensaje(msg: &[u8], firma: &[u8; 65]) -> Option<[u8; 20]> {
    let sig = Signature::from_slice(&firma[..64]).ok()?;
    let rec = RecoveryId::from_byte(firma[64].checked_sub(27)?)?;
    VerifyingKey::recover_from_prehash(&hash_mensaje(msg), &sig, rec).ok().map(|pk| direccion_eth_de(&pk))
}
