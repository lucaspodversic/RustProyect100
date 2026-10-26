//! Intérprete EVM simplificado.
//!
//! Modelo de ejecución (igual que la EVM real, recortado):
//! - **Pila** de palabras U256 (máx. 1024).
//! - **Memoria** lineal byte a byte que crece en palabras de 32 bytes y cobra gas
//!   por expansión (`3·w + w²/512`).
//! - **Storage** persistente clave→valor por contrato; sólo se confirma si la
//!   ejecución termina bien (REVERT o error ⇒ se descarta todo).
//! - **Gas**: cada opcode cobra un costo fijo + dinámico; quedarse sin gas aborta.
//! - **Saltos** sólo a `JUMPDEST` válidos (no dentro de los datos de un PUSH).

use crate::opcodes::*;
use crate::u256::U256;
use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use thiserror::Error;

pub const MAX_PILA: usize = 1024;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ErrorEvm {
    #[error("pila vacía (underflow) en pc={0}")]
    PilaVacia(usize),
    #[error("desbordamiento de pila en pc={0}")]
    PilaLlena(usize),
    #[error("sin gas")]
    SinGas,
    #[error("salto inválido a {0}")]
    SaltoInvalido(usize),
    #[error("opcode inválido 0x{0:02x}")]
    OpcodeInvalido(u8),
}

/// Contexto de la llamada (lo que en Solidity es `msg.*`).
#[derive(Debug, Clone, Default)]
pub struct Entorno {
    pub remitente: U256,
    pub valor: U256,
    pub calldata: Vec<u8>,
    pub limite_gas: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Log {
    pub topics: Vec<U256>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Estado {
    /// STOP o RETURN.
    Exito,
    /// REVERT explícito (devuelve datos, reembolsa el gas restante).
    Revertido,
    /// Error excepcional (consume todo el gas).
    Fallo(ErrorEvm),
}

#[derive(Debug, Clone)]
pub struct Resultado {
    pub estado: Estado,
    pub salida: Vec<u8>,
    pub gas_usado: u64,
    pub logs: Vec<Log>,
}

pub type Storage = HashMap<U256, U256>;

/// Calcula los destinos de salto válidos saltando los datos inmediatos de PUSH.
pub fn analizar_jumpdests(code: &[u8]) -> Vec<bool> {
    let mut v = vec![false; code.len()];
    let mut i = 0;
    while i < code.len() {
        let op = code[i];
        if op == JUMPDEST {
            v[i] = true;
        }
        i += 1 + bytes_push(op);
    }
    v
}

fn costo_memoria(palabras: u64) -> u64 {
    3 * palabras + palabras * palabras / 512
}

struct Maquina<'a> {
    code: &'a [u8],
    validos: Vec<bool>,
    pila: Vec<U256>,
    mem: Vec<u8>,
    pc: usize,
    gas: u64,
    env: &'a Entorno,
    storage: Storage,
    logs: Vec<Log>,
}

impl Maquina<'_> {
    fn cobrar(&mut self, g: u64) -> Result<(), ErrorEvm> {
        self.gas = self.gas.checked_sub(g).ok_or(ErrorEvm::SinGas)?;
        Ok(())
    }

    fn pop(&mut self) -> Result<U256, ErrorEvm> {
        self.pila.pop().ok_or(ErrorEvm::PilaVacia(self.pc))
    }

    fn push(&mut self, v: U256) -> Result<(), ErrorEvm> {
        if self.pila.len() >= MAX_PILA {
            return Err(ErrorEvm::PilaLlena(self.pc));
        }
        self.pila.push(v);
        Ok(())
    }

    /// Asegura memoria para [off, off+len) cobrando la expansión.
    fn expandir(&mut self, off: U256, len: U256) -> Result<(usize, usize), ErrorEvm> {
        if len.is_zero() {
            return Ok((0, 0));
        }
        let (o, l) = (off.as_usize_sat(), len.as_usize_sat());
        // Límite práctico: offsets absurdos se tratan como "sin gas".
        let fin = o.checked_add(l).filter(|f| *f <= 1 << 24).ok_or(ErrorEvm::SinGas)?;
        let nuevas = fin.div_ceil(32) as u64;
        let actuales = (self.mem.len() / 32) as u64;
        if nuevas > actuales {
            self.cobrar(costo_memoria(nuevas) - costo_memoria(actuales))?;
            self.mem.resize(nuevas as usize * 32, 0);
        }
        Ok((o, l))
    }

    fn binop(&mut self, f: impl Fn(U256, U256) -> U256) -> Result<(), ErrorEvm> {
        let a = self.pop()?;
        let b = self.pop()?;
        self.push(f(a, b))
    }

    /// Ejecuta un paso. Devuelve `Some(estado, salida)` si terminó.
    fn paso(&mut self) -> Result<Option<(Estado, Vec<u8>)>, ErrorEvm> {
        let Some(&op) = self.code.get(self.pc) else {
            return Ok(Some((Estado::Exito, vec![]))); // fin del código = STOP
        };
        self.cobrar(gas_base(op).ok_or(ErrorEvm::OpcodeInvalido(op))?)?;
        let b = |x: bool| if x { U256::ONE } else { U256::ZERO };
        let mut siguiente = self.pc + 1;
        match op {
            STOP => return Ok(Some((Estado::Exito, vec![]))),
            ADD => self.binop(|a, c| a.wrapping_add(c))?,
            MUL => self.binop(|a, c| a.wrapping_mul(c))?,
            SUB => self.binop(|a, c| a.wrapping_sub(c))?,
            DIV => self.binop(|a, c| a.div_rem(c).0)?,
            MOD => self.binop(|a, c| a.div_rem(c).1)?,
            EXP => {
                let base = self.pop()?;
                let e = self.pop()?;
                // 50 de gas por cada byte del exponente.
                self.cobrar(50 * e.bits().div_ceil(8) as u64)?;
                self.push(base.wrapping_pow(e))?;
            }
            LT => self.binop(|a, c| b(a < c))?,
            GT => self.binop(|a, c| b(a > c))?,
            EQ => self.binop(|a, c| b(a == c))?,
            ISZERO => {
                let a = self.pop()?;
                self.push(b(a.is_zero()))?;
            }
            AND => self.binop(|a, c| a & c)?,
            OR => self.binop(|a, c| a | c)?,
            XOR => self.binop(|a, c| a ^ c)?,
            NOT => {
                let a = self.pop()?;
                self.push(!a)?;
            }
            BYTE => self.binop(|i, x| match i.as_u64() {
                Some(i) if i < 32 => U256::from_u64(x.to_be_bytes()[i as usize] as u64),
                _ => U256::ZERO,
            })?,
            SHL => self.binop(|s, x| x.shl(s.as_usize_sat()))?,
            SHR => self.binop(|s, x| x.shr(s.as_usize_sat()))?,
            KECCAK256 => {
                let (off, len) = (self.pop()?, self.pop()?);
                let (o, l) = self.expandir(off, len)?;
                self.cobrar(6 * l.div_ceil(32) as u64)?;
                let h = Keccak256::digest(&self.mem[o..o + l]);
                self.push(U256::from_be_bytes(&h))?;
            }
            CALLER => self.push(self.env.remitente)?,
            CALLVALUE => self.push(self.env.valor)?,
            CALLDATALOAD => {
                let i = self.pop()?.as_usize_sat();
                let mut w = [0u8; 32];
                for (k, x) in w.iter_mut().enumerate() {
                    *x = i.checked_add(k).and_then(|j| self.env.calldata.get(j)).copied().unwrap_or(0);
                }
                self.push(U256::from_be_bytes(&w))?;
            }
            CALLDATASIZE => self.push(U256::from_u64(self.env.calldata.len() as u64))?,
            POP => {
                self.pop()?;
            }
            MLOAD => {
                let off = self.pop()?;
                let (o, _) = self.expandir(off, U256::from_u64(32))?;
                let v = U256::from_be_bytes(&self.mem[o..o + 32]);
                self.push(v)?;
            }
            MSTORE => {
                let (off, v) = (self.pop()?, self.pop()?);
                let (o, _) = self.expandir(off, U256::from_u64(32))?;
                self.mem[o..o + 32].copy_from_slice(&v.to_be_bytes());
            }
            MSTORE8 => {
                let (off, v) = (self.pop()?, self.pop()?);
                let (o, _) = self.expandir(off, U256::ONE)?;
                self.mem[o] = v.0[0] as u8;
            }
            SLOAD => {
                let k = self.pop()?;
                let v = self.storage.get(&k).copied().unwrap_or_default();
                self.push(v)?;
            }
            SSTORE => {
                let (k, v) = (self.pop()?, self.pop()?);
                let previo = self.storage.get(&k).copied().unwrap_or_default();
                // Simplificación del EIP-2200: poner un slot de 0 a ≠0 es caro.
                self.cobrar(if previo.is_zero() && !v.is_zero() { 20_000 } else { 5_000 })?;
                if v.is_zero() {
                    self.storage.remove(&k);
                } else {
                    self.storage.insert(k, v);
                }
            }
            JUMP => {
                let d = self.pop()?.as_usize_sat();
                siguiente = self.destino(d)?;
            }
            JUMPI => {
                let (d, cond) = (self.pop()?, self.pop()?);
                if !cond.is_zero() {
                    siguiente = self.destino(d.as_usize_sat())?;
                }
            }
            PC => self.push(U256::from_u64(self.pc as u64))?,
            MSIZE => self.push(U256::from_u64(self.mem.len() as u64))?,
            GAS => self.push(U256::from_u64(self.gas))?,
            JUMPDEST => {}
            PUSH0..=PUSH32 => {
                let n = bytes_push(op);
                // Si el código se corta, los bytes faltantes valen 0 (por la derecha).
                let mut datos = vec![0u8; n];
                for (k, x) in datos.iter_mut().enumerate() {
                    *x = self.code.get(self.pc + 1 + k).copied().unwrap_or(0);
                }
                self.push(U256::from_be_bytes(&datos))?;
                siguiente = self.pc + 1 + n;
            }
            DUP1..=DUP16 => {
                let n = (op - DUP1 + 1) as usize;
                let v = *self.pila.len().checked_sub(n).and_then(|i| self.pila.get(i)).ok_or(ErrorEvm::PilaVacia(self.pc))?;
                self.push(v)?;
            }
            SWAP1..=SWAP16 => {
                let n = (op - SWAP1 + 1) as usize;
                let len = self.pila.len();
                if len < n + 1 {
                    return Err(ErrorEvm::PilaVacia(self.pc));
                }
                self.pila.swap(len - 1, len - 1 - n);
            }
            LOG0..=LOG4 => {
                let n = (op - LOG0) as usize;
                let (off, len) = (self.pop()?, self.pop()?);
                let mut topics = vec![];
                for _ in 0..n {
                    topics.push(self.pop()?);
                }
                let (o, l) = self.expandir(off, len)?;
                self.cobrar(375 * n as u64 + 8 * l as u64)?;
                self.logs.push(Log { topics, data: self.mem[o..o + l].to_vec() });
            }
            RETURN | REVERT => {
                let (off, len) = (self.pop()?, self.pop()?);
                let (o, l) = self.expandir(off, len)?;
                let datos = self.mem[o..o + l].to_vec();
                let e = if op == RETURN { Estado::Exito } else { Estado::Revertido };
                return Ok(Some((e, datos)));
            }
            otro => return Err(ErrorEvm::OpcodeInvalido(otro)),
        }
        self.pc = siguiente;
        Ok(None)
    }

    fn destino(&self, d: usize) -> Result<usize, ErrorEvm> {
        if self.validos.get(d).copied().unwrap_or(false) {
            Ok(d)
        } else {
            Err(ErrorEvm::SaltoInvalido(d))
        }
    }
}

/// Ejecuta `code` sobre `storage`. El storage sólo se modifica si termina en éxito.
pub fn ejecutar(code: &[u8], env: &Entorno, storage: &mut Storage) -> Resultado {
    let mut m = Maquina {
        code,
        validos: analizar_jumpdests(code),
        pila: Vec::new(),
        mem: Vec::new(),
        pc: 0,
        gas: env.limite_gas,
        env,
        storage: storage.clone(), // copia de trabajo (snapshot para poder revertir)
        logs: vec![],
    };
    loop {
        match m.paso() {
            Ok(None) => continue,
            Ok(Some((estado, salida))) => {
                let gas_usado = env.limite_gas - m.gas;
                if estado == Estado::Exito {
                    *storage = m.storage;
                    return Resultado { estado, salida, gas_usado, logs: m.logs };
                }
                // REVERT: devuelve datos pero descarta storage y logs.
                return Resultado { estado, salida, gas_usado, logs: vec![] };
            }
            Err(e) => {
                // Error excepcional: se consume todo el gas.
                return Resultado { estado: Estado::Fallo(e), salida: vec![], gas_usado: env.limite_gas, logs: vec![] };
            }
        }
    }
}
