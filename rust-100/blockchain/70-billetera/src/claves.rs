//! Generación e importación de claves y derivación de direcciones.
//!
//! - **Ethereum**: `keccak256(pubkey_sin_comprimir[1..])[12..]` → 20 bytes, y
//!   checksum EIP-55 (mayúsculas según los nibbles del keccak de la dirección).
//! - **Bitcoin P2PKH**: `base58check(0x00 || ripemd160(sha256(pubkey_comprimida)))`.

use k256::ecdsa::{SigningKey, VerifyingKey};
use ripemd::Ripemd160;
use sha2::{Digest as _, Sha256};
use sha3::Keccak256;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErrorClave {
    #[error("hex inválido")]
    Hex,
    #[error("la clave privada debe tener 32 bytes y estar en el rango de la curva")]
    ClaveInvalida,
    #[error("dirección inválida")]
    Direccion,
}

/// Par de claves de la billetera. La clave privada nunca se imprime por `Debug`.
pub struct ParClaves {
    privada: SigningKey,
}

impl std::fmt::Debug for ParClaves {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Nunca filtrar la clave privada en logs.
        write!(f, "ParClaves {{ direccion: {} }}", self.direccion_eth())
    }
}

pub fn keccak256(d: &[u8]) -> [u8; 32] {
    Keccak256::digest(d).into()
}

impl ParClaves {
    /// Genera una clave nueva con la entropía del SO (`OsRng`).
    pub fn generar() -> Self {
        Self { privada: SigningKey::random(&mut rand::rngs::OsRng) }
    }

    /// Importa desde hex (con o sin `0x`).
    pub fn desde_hex(s: &str) -> Result<Self, ErrorClave> {
        let b = hex::decode(s.trim().trim_start_matches("0x")).map_err(|_| ErrorClave::Hex)?;
        Self::desde_bytes(&b)
    }

    pub fn desde_bytes(b: &[u8]) -> Result<Self, ErrorClave> {
        if b.len() != 32 {
            return Err(ErrorClave::ClaveInvalida);
        }
        SigningKey::from_slice(b).map(|privada| Self { privada }).map_err(|_| ErrorClave::ClaveInvalida)
    }

    /// Exporta la clave privada en hex (¡manejar con cuidado!).
    pub fn exportar_hex(&self) -> String {
        hex::encode(self.privada.to_bytes())
    }

    pub fn firmante(&self) -> &SigningKey {
        &self.privada
    }

    pub fn publica(&self) -> VerifyingKey {
        *self.privada.verifying_key()
    }

    /// Clave pública comprimida (33 bytes, prefijo 02/03).
    pub fn publica_comprimida(&self) -> Vec<u8> {
        self.publica().to_encoded_point(true).as_bytes().to_vec()
    }

    /// Dirección Ethereum de 20 bytes.
    pub fn direccion_eth_bytes(&self) -> [u8; 20] {
        direccion_eth_de(&self.publica())
    }

    /// Dirección Ethereum con checksum EIP-55.
    pub fn direccion_eth(&self) -> String {
        checksum_eip55(&self.direccion_eth_bytes())
    }

    /// Dirección Bitcoin P2PKH (mainnet) a partir de la clave comprimida.
    pub fn direccion_btc(&self) -> String {
        let h160 = Ripemd160::digest(Sha256::digest(self.publica_comprimida()));
        let mut payload = vec![0x00u8];
        payload.extend_from_slice(&h160);
        base58check(&payload)
    }
}

/// Deriva la dirección Ethereum de una clave pública.
pub fn direccion_eth_de(pk: &VerifyingKey) -> [u8; 20] {
    let p = pk.to_encoded_point(false); // 0x04 || X || Y
    let h = keccak256(&p.as_bytes()[1..]);
    h[12..].try_into().unwrap()
}

/// EIP-55: cada letra hex va en mayúscula si el nibble correspondiente del
/// keccak256 de la dirección en minúsculas es >= 8. Detecta typos.
pub fn checksum_eip55(dir: &[u8; 20]) -> String {
    let minus = hex::encode(dir);
    let h = keccak256(minus.as_bytes());
    let mut s = String::from("0x");
    for (i, c) in minus.chars().enumerate() {
        let nibble = (h[i / 2] >> (if i % 2 == 0 { 4 } else { 0 })) & 0x0f;
        s.push(if c.is_ascii_alphabetic() && nibble >= 8 { c.to_ascii_uppercase() } else { c });
    }
    s
}

/// Valida una dirección Ethereum. Si tiene mayúsculas y minúsculas mezcladas,
/// exige que el checksum EIP-55 sea correcto.
pub fn validar_direccion_eth(s: &str) -> Result<[u8; 20], ErrorClave> {
    let h = s.strip_prefix("0x").ok_or(ErrorClave::Direccion)?;
    if h.len() != 40 {
        return Err(ErrorClave::Direccion);
    }
    let b: [u8; 20] = hex::decode(h).map_err(|_| ErrorClave::Direccion)?.try_into().unwrap();
    let mixto = h.chars().any(|c| c.is_ascii_uppercase()) && h.chars().any(|c| c.is_ascii_lowercase());
    if mixto && checksum_eip55(&b) != s {
        return Err(ErrorClave::Direccion);
    }
    Ok(b)
}

/// Base58Check: payload || primeros 4 bytes de sha256(sha256(payload)).
pub fn base58check(payload: &[u8]) -> String {
    let chk = Sha256::digest(Sha256::digest(payload));
    let mut v = payload.to_vec();
    v.extend_from_slice(&chk[..4]);
    bs58::encode(v).into_string()
}
