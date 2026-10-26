//! # Staking estilo Solana (proyecto 68)
//!
//! Anchor/solana-cli no se instalan de forma confiable en este entorno
//! (toolchain BPF + ~GB de descargas), así que el contrato se implementa como
//! **lógica Rust pura sobre un mini-runtime que reproduce el modelo de cuentas
//! de Solana** (cuentas con dueño, PDAs, firmantes, instrucciones
//! serializadas, atomicidad y conservación de lamports). En `anchor/` está el
//! mismo programa escrito para Anchor, listo para `anchor build`.

pub mod cliente;
pub mod estado;
pub mod programa;
pub mod runtime;
