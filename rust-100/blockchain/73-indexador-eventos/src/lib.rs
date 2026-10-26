//! # Indexador de eventos on-chain (proyecto 73)
//!
//! - [`modelo`]: logs estilo EVM, topics keccak y decodificación ABI simplificada.
//! - [`cadena_mock`]: cadena simulada con reorgs (en producción: JSON-RPC).
//! - [`indexador`]: ingesta con confirmaciones, detección/rollback de reorgs,
//!   índices por dirección, consultas paginadas, agregados y checkpoint JSON.

pub mod cadena_mock;
pub mod indexador;
pub mod modelo;
