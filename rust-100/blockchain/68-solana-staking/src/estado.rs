//! Estructuras de estado del programa y su serialización binaria.
//!
//! Anchor usa Borsh con un **discriminador** de 8 bytes al inicio de cada
//! cuenta para distinguir tipos. Replicamos esa idea a mano (sin dependencias):
//! enteros little-endian de ancho fijo y un discriminador derivado del nombre.

use crate::runtime::Pubkey;
use sha2::{Digest, Sha256};

/// Discriminador estilo Anchor: primeros 8 bytes de `sha256("account:<Nombre>")`.
pub fn discriminador(nombre: &str) -> [u8; 8] {
    let d = Sha256::digest(format!("account:{nombre}").as_bytes());
    d[..8].try_into().unwrap()
}

/// Lector secuencial de bytes con chequeo de límites.
pub struct Lector<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Lector<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn tomar(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.buf.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(s)
    }
    pub fn u8(&mut self) -> Option<u8> {
        self.tomar(1).map(|s| s[0])
    }
    pub fn u64(&mut self) -> Option<u64> {
        self.tomar(8).map(|s| u64::from_le_bytes(s.try_into().unwrap()))
    }
    pub fn i64(&mut self) -> Option<i64> {
        self.tomar(8).map(|s| i64::from_le_bytes(s.try_into().unwrap()))
    }
    pub fn u128(&mut self) -> Option<u128> {
        self.tomar(16).map(|s| u128::from_le_bytes(s.try_into().unwrap()))
    }
    pub fn pubkey(&mut self) -> Option<Pubkey> {
        self.tomar(32).map(|s| Pubkey(s.try_into().unwrap()))
    }
    pub fn disc(&mut self) -> Option<[u8; 8]> {
        self.tomar(8).map(|s| s.try_into().unwrap())
    }
}

/// Configuración global del pool de staking (una PDA por autoridad).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pool {
    /// Quién puede pausar el pool o cambiar la tasa.
    pub autoridad: Pubkey,
    /// Lamports de recompensa por cada lamport stakeado y por segundo,
    /// escalado por `ESCALA` (aritmética de punto fijo).
    pub tasa_por_segundo: u64,
    /// Período mínimo de bloqueo antes de poder retirar (segundos).
    pub bloqueo_segundos: i64,
    /// Total stakeado actualmente.
    pub total_stakeado: u64,
    /// Si está pausado, no se aceptan nuevos depósitos.
    pub pausado: bool,
    /// Bump de la PDA (se guarda para no recalcularlo).
    pub bump: u8,
}

/// Escala de punto fijo para la tasa (1e9 = 100% por segundo).
pub const ESCALA: u128 = 1_000_000_000;

impl Pool {
    pub const TAMANO: usize = 8 + 32 + 8 + 8 + 8 + 1 + 1;

    pub fn serializar(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(Self::TAMANO);
        v.extend(discriminador("Pool"));
        v.extend(self.autoridad.0);
        v.extend(self.tasa_por_segundo.to_le_bytes());
        v.extend(self.bloqueo_segundos.to_le_bytes());
        v.extend(self.total_stakeado.to_le_bytes());
        v.push(self.pausado as u8);
        v.push(self.bump);
        v
    }

    pub fn deserializar(d: &[u8]) -> Option<Self> {
        let mut l = Lector::new(d);
        if l.disc()? != discriminador("Pool") {
            return None; // tipo de cuenta equivocado: defensa contra "type cosplay"
        }
        Some(Self {
            autoridad: l.pubkey()?,
            tasa_por_segundo: l.u64()?,
            bloqueo_segundos: l.i64()?,
            total_stakeado: l.u64()?,
            pausado: l.u8()? != 0,
            bump: l.u8()?,
        })
    }
}

/// Posición de staking de un usuario (PDA `["stake", pool, usuario]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posicion {
    pub dueno: Pubkey,
    pub pool: Pubkey,
    pub monto: u64,
    /// Momento del último cálculo de recompensas.
    pub ultimo_calculo: i64,
    /// Momento del primer depósito (para el bloqueo).
    pub desde: i64,
    /// Recompensas devengadas aún no cobradas (punto fijo × ESCALA).
    pub pendiente: u128,
    pub bump: u8,
}

impl Posicion {
    pub const TAMANO: usize = 8 + 32 + 32 + 8 + 8 + 8 + 16 + 1;

    pub fn serializar(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(Self::TAMANO);
        v.extend(discriminador("Posicion"));
        v.extend(self.dueno.0);
        v.extend(self.pool.0);
        v.extend(self.monto.to_le_bytes());
        v.extend(self.ultimo_calculo.to_le_bytes());
        v.extend(self.desde.to_le_bytes());
        v.extend(self.pendiente.to_le_bytes());
        v.push(self.bump);
        v
    }

    pub fn deserializar(d: &[u8]) -> Option<Self> {
        let mut l = Lector::new(d);
        if l.disc()? != discriminador("Posicion") {
            return None;
        }
        Some(Self {
            dueno: l.pubkey()?,
            pool: l.pubkey()?,
            monto: l.u64()?,
            ultimo_calculo: l.i64()?,
            desde: l.i64()?,
            pendiente: l.u128()?,
            bump: l.u8()?,
        })
    }

    /// Devenga recompensas hasta `ahora` usando la tasa del pool.
    pub fn devengar(&mut self, tasa: u64, ahora: i64) {
        let dt = (ahora - self.ultimo_calculo).max(0) as u128;
        self.pendiente += self.monto as u128 * tasa as u128 * dt;
        self.ultimo_calculo = ahora;
    }
}
