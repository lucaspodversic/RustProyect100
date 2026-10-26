//! # Escrow con atomic swap entre dos partes (proyecto 75)
//!
//! - [`cadena`]: cadena simulada con contratos HTLC.
//! - [`swap`]: protocolo de atomic swap cross-chain (Alicia ⇄ Beto).
//! - [`escrow`]: escrow 2-de-3 con árbitro para casos no verificables on-chain.

pub mod cadena;
pub mod escrow;
pub mod swap;
