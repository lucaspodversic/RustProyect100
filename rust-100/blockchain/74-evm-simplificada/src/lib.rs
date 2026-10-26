//! # EVM simplificada (proyecto 74)
//!
//! - [`u256`]: aritmética de 256 bits hecha a mano (mod 2^256).
//! - [`opcodes`]: tabla de ~60 opcodes, gas base y mnemónicos.
//! - [`vm`]: intérprete (pila, memoria, storage, gas, saltos, logs, RETURN/REVERT).
//! - [`asm`]: ensamblador con etiquetas para escribir programas legibles.

pub mod asm;
pub mod opcodes;
pub mod u256;
pub mod vm;
