//! Escrow simple con **árbitro** (2 de 3): comprador deposita; los fondos se
//! liberan al vendedor o vuelven al comprador cuando dos de las tres partes
//! (comprador, vendedor, árbitro) votan lo mismo. Útil cuando la entrega es
//! un bien físico que no se puede verificar on-chain con un hashlock.

use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Decision {
    Liberar,
    Devolver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoEscrow {
    Fondeado,
    Liberado,
    Devuelto,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErrorEscrow {
    #[error("no es parte del escrow")]
    NoEsParte,
    #[error("escrow ya cerrado")]
    Cerrado,
}

pub struct Escrow {
    pub comprador: String,
    pub vendedor: String,
    pub arbitro: String,
    pub monto: u64,
    pub estado: EstadoEscrow,
    votos: HashMap<String, Decision>,
}

impl Escrow {
    pub fn nuevo(comprador: &str, vendedor: &str, arbitro: &str, monto: u64) -> Self {
        Self { comprador: comprador.into(), vendedor: vendedor.into(), arbitro: arbitro.into(), monto, estado: EstadoEscrow::Fondeado, votos: HashMap::new() }
    }

    /// Registra (o cambia) el voto; devuelve `Some((beneficiario, monto))` al resolverse.
    pub fn votar(&mut self, quien: &str, d: Decision) -> Result<Option<(String, u64)>, ErrorEscrow> {
        if self.estado != EstadoEscrow::Fondeado {
            return Err(ErrorEscrow::Cerrado);
        }
        if ![&self.comprador, &self.vendedor, &self.arbitro].contains(&&quien.to_string()) {
            return Err(ErrorEscrow::NoEsParte);
        }
        self.votos.insert(quien.into(), d);
        let n = self.votos.values().filter(|v| **v == d).count();
        if n >= 2 {
            let (estado, quien) = match d {
                Decision::Liberar => (EstadoEscrow::Liberado, self.vendedor.clone()),
                Decision::Devolver => (EstadoEscrow::Devuelto, self.comprador.clone()),
            };
            self.estado = estado;
            return Ok(Some((quien, self.monto)));
        }
        Ok(None)
    }
}
