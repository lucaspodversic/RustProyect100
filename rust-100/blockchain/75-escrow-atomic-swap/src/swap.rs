//! Protocolo de **atomic swap** entre Alicia (tiene monedas en la cadena A) y
//! Beto (tiene monedas en la cadena B):
//!
//! 1. Alicia genera un secreto `s` y publica `H = sha256(s)`.
//! 2. Alicia bloquea `x` en A para Beto con hashlock H y timelock **T1** (largo).
//! 3. Beto verifica el HTLC de A y bloquea `y` en B para Alicia con el mismo H y
//!    timelock **T2 < T1** (corto, con margen de seguridad).
//! 4. Alicia reclama en B revelando `s` (antes de T2).
//! 5. Beto lee `s` de la cadena B y reclama en A (antes de T1).
//!
//! Atomicidad: o ambos cobran, o ambos recuperan sus fondos tras los timelocks.
//! Que T2 sea menor que T1 es clave: si fuera al revés, Alicia podría esperar
//! a que venza el de A, reembolsarse y aun así reclamar en B.

use crate::cadena::{sha256, Cadena, ErrorCadena, Hash};
use rand::RngCore;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErrorSwap {
    #[error("el HTLC de la contraparte no cumple lo acordado: {0}")]
    HtlcIncorrecto(&'static str),
    #[error("margen de timelock insuficiente")]
    MargenInsuficiente,
    #[error("el secreto todavía no fue revelado")]
    SecretoNoRevelado,
    #[error(transparent)]
    Cadena(#[from] ErrorCadena),
}

/// Términos acordados off-chain.
#[derive(Debug, Clone)]
pub struct Acuerdo {
    pub alicia: String,
    pub beto: String,
    pub monto_a: u64,
    pub monto_b: u64,
    /// Duración (en bloques) del timelock de Alicia en A.
    pub t1: u64,
    /// Duración del timelock de Beto en B (debe ser < t1 - margen).
    pub t2: u64,
    /// Margen mínimo que Beto exige para tener tiempo de reclamar en A.
    pub margen: u64,
}

pub struct Alicia {
    secreto: [u8; 32],
    pub hashlock: Hash,
}

impl Alicia {
    pub fn nueva() -> Self {
        let mut s = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut s);
        Self { hashlock: sha256(&s), secreto: s }
    }

    /// Paso 2.
    pub fn bloquear_en_a(&self, a: &mut Cadena, ac: &Acuerdo) -> Result<usize, ErrorSwap> {
        Ok(a.bloquear(&ac.alicia, &ac.beto, ac.monto_a, self.hashlock, a.altura + ac.t1)?)
    }

    /// Paso 4: reclamar en B (revela el secreto).
    pub fn reclamar_en_b(&self, b: &mut Cadena, id_b: usize, ac: &Acuerdo) -> Result<(), ErrorSwap> {
        Ok(b.reclamar(id_b, &ac.alicia, &self.secreto)?)
    }
}

/// Paso 3: Beto verifica TODO antes de arriesgar sus fondos.
pub fn beto_verificar_y_bloquear(a: &Cadena, id_a: usize, b: &mut Cadena, ac: &Acuerdo) -> Result<usize, ErrorSwap> {
    let h = a.htlcs.get(id_a).ok_or(ErrorCadena::NoExiste)?;
    if h.receptor != ac.beto {
        return Err(ErrorSwap::HtlcIncorrecto("receptor"));
    }
    if h.monto != ac.monto_a {
        return Err(ErrorSwap::HtlcIncorrecto("monto"));
    }
    if h.estado != crate::cadena::EstadoHtlc::Bloqueado {
        return Err(ErrorSwap::HtlcIncorrecto("estado"));
    }
    // Tiempo restante en A vs. lo que va a bloquear en B (asumimos cadenas de igual ritmo).
    let restante_a = h.timelock.saturating_sub(a.altura);
    if ac.t2 + ac.margen > restante_a {
        return Err(ErrorSwap::MargenInsuficiente);
    }
    Ok(b.bloquear(&ac.beto, &ac.alicia, ac.monto_b, h.hashlock, b.altura + ac.t2)?)
}

/// Paso 5: Beto extrae el secreto publicado en B y cobra en A.
pub fn beto_reclamar_en_a(a: &mut Cadena, id_a: usize, b: &Cadena, id_b: usize, ac: &Acuerdo) -> Result<(), ErrorSwap> {
    let s = b.htlcs.get(id_b).and_then(|h| h.preimagen.clone()).ok_or(ErrorSwap::SecretoNoRevelado)?;
    Ok(a.reclamar(id_a, &ac.beto, &s)?)
}
