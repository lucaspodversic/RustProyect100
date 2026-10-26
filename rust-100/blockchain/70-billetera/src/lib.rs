//! # Billetera de criptomonedas (proyecto 70)
//!
//! Criptografía real con crates estándar de RustCrypto:
//! - [`claves`]: generación (OsRng), importación/exportación, direcciones ETH (EIP-55) y BTC (P2PKH).
//! - [`rlp`]: codificador/decodificador RLP.
//! - [`tx`]: transacciones EIP-155 firmadas, ecrecover y firma de mensajes EIP-191.

pub mod claves;
pub mod rlp;
pub mod tx;
