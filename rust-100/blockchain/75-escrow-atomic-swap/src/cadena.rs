//! Una cadena simulada con saldos, altura de bloque y contratos **HTLC**
//! (Hashed TimeLock Contract):
//!
//! - `bloquear`: el emisor deposita `monto` para el `receptor`, condicionado a
//!   que éste presente `s` tal que `sha256(s) == hashlock` **antes** de `timelock`.
//! - `reclamar`: el receptor revela el secreto y cobra. ¡El secreto queda
//!   público en la cadena! (de eso depende el swap).
//! - `reembolsar`: si venció el timelock y nadie reclamó, el emisor recupera.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use thiserror::Error;

pub type Hash = [u8; 32];

pub fn sha256(x: &[u8]) -> Hash {
    Sha256::digest(x).into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoHtlc {
    Bloqueado,
    Reclamado,
    Reembolsado,
}

#[derive(Debug, Clone)]
pub struct Htlc {
    pub emisor: String,
    pub receptor: String,
    pub monto: u64,
    pub hashlock: Hash,
    pub timelock: u64,
    pub estado: EstadoHtlc,
    /// Secreto revelado al reclamar (visible para cualquiera).
    pub preimagen: Option<Vec<u8>>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErrorCadena {
    #[error("saldo insuficiente")]
    SaldoInsuficiente,
    #[error("HTLC inexistente")]
    NoExiste,
    #[error("HTLC ya resuelto")]
    YaResuelto,
    #[error("preimagen incorrecta")]
    PreimagenIncorrecta,
    #[error("timelock vencido")]
    Vencido,
    #[error("timelock aún no vence")]
    NoVencido,
    #[error("no autorizado")]
    NoAutorizado,
    #[error("parámetros inválidos")]
    Invalido,
}

#[derive(Debug, Default)]
pub struct Cadena {
    pub nombre: String,
    pub altura: u64,
    pub saldos: HashMap<String, u64>,
    pub htlcs: Vec<Htlc>,
}

impl Cadena {
    pub fn nueva(nombre: &str, saldos: &[(&str, u64)]) -> Self {
        Self { nombre: nombre.into(), saldos: saldos.iter().map(|(k, v)| (k.to_string(), *v)).collect(), ..Default::default() }
    }

    pub fn saldo(&self, q: &str) -> u64 {
        self.saldos.get(q).copied().unwrap_or(0)
    }

    pub fn avanzar(&mut self, bloques: u64) {
        self.altura += bloques;
    }

    /// Crea un HTLC y devuelve su id. Los fondos quedan en custodia del contrato.
    pub fn bloquear(&mut self, emisor: &str, receptor: &str, monto: u64, hashlock: Hash, timelock: u64) -> Result<usize, ErrorCadena> {
        if monto == 0 || timelock <= self.altura || emisor == receptor {
            return Err(ErrorCadena::Invalido);
        }
        let s = self.saldos.get_mut(emisor).ok_or(ErrorCadena::SaldoInsuficiente)?;
        *s = s.checked_sub(monto).ok_or(ErrorCadena::SaldoInsuficiente)?;
        self.htlcs.push(Htlc { emisor: emisor.into(), receptor: receptor.into(), monto, hashlock, timelock, estado: EstadoHtlc::Bloqueado, preimagen: None });
        Ok(self.htlcs.len() - 1)
    }

    pub fn reclamar(&mut self, id: usize, quien: &str, preimagen: &[u8]) -> Result<(), ErrorCadena> {
        let altura = self.altura;
        let h = self.htlcs.get_mut(id).ok_or(ErrorCadena::NoExiste)?;
        if h.estado != EstadoHtlc::Bloqueado {
            return Err(ErrorCadena::YaResuelto);
        }
        if quien != h.receptor {
            return Err(ErrorCadena::NoAutorizado);
        }
        if altura >= h.timelock {
            return Err(ErrorCadena::Vencido);
        }
        if sha256(preimagen) != h.hashlock {
            return Err(ErrorCadena::PreimagenIncorrecta);
        }
        h.estado = EstadoHtlc::Reclamado;
        h.preimagen = Some(preimagen.to_vec());
        let (r, m) = (h.receptor.clone(), h.monto);
        *self.saldos.entry(r).or_insert(0) += m;
        Ok(())
    }

    /// Cualquiera puede disparar el reembolso, pero los fondos van al emisor.
    pub fn reembolsar(&mut self, id: usize) -> Result<(), ErrorCadena> {
        let altura = self.altura;
        let h = self.htlcs.get_mut(id).ok_or(ErrorCadena::NoExiste)?;
        if h.estado != EstadoHtlc::Bloqueado {
            return Err(ErrorCadena::YaResuelto);
        }
        if altura < h.timelock {
            return Err(ErrorCadena::NoVencido);
        }
        h.estado = EstadoHtlc::Reembolsado;
        let (e, m) = (h.emisor.clone(), h.monto);
        *self.saldos.entry(e).or_insert(0) += m;
        Ok(())
    }

    /// Suma de saldos + fondos en custodia: debe conservarse siempre.
    pub fn oferta_total(&self) -> u64 {
        self.saldos.values().sum::<u64>() + self.htlcs.iter().filter(|h| h.estado == EstadoHtlc::Bloqueado).map(|h| h.monto).sum::<u64>()
    }
}
