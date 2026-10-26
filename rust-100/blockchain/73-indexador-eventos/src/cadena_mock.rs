//! Cadena simulada que produce bloques con logs y permite forzar **reorgs**.

use crate::modelo::*;
use sha3::{Digest, Keccak256};

/// Fuente de bloques (en producción: cliente JSON-RPC `eth_getBlockByNumber` + `eth_getLogs`).
pub trait FuenteBloques {
    fn ultimo(&self) -> u64;
    fn bloque(&self, n: u64) -> Option<BloqueConLogs>;
}

#[derive(Default)]
pub struct CadenaMock {
    pub bloques: Vec<BloqueConLogs>,
    /// Logs pendientes para el próximo bloque.
    pendientes: Vec<(Direccion, Vec<H256>, Vec<u8>)>,
    /// Sal para que bloques "alternativos" tengan otro hash.
    pub rama: u8,
}

impl CadenaMock {
    pub fn nueva() -> Self {
        let mut c = Self::default();
        c.minar(); // génesis
        c
    }

    pub fn emitir(&mut self, contrato: Direccion, topics: Vec<H256>, data: Vec<u8>) {
        self.pendientes.push((contrato, topics, data));
    }

    pub fn transfer(&mut self, token: Direccion, de: Direccion, para: Direccion, valor: u128) {
        self.emitir(token, vec![topic_de(FIRMA_TRANSFER), dir_a_topic(&de), dir_a_topic(&para)], u128_a_u256(valor));
    }

    pub fn listado(&mut self, market: Direccion, id: u64, vendedor: Direccion, precio: u128) {
        let mut tid = [0u8; 32];
        tid[24..].copy_from_slice(&id.to_be_bytes());
        self.emitir(market, vec![topic_de(FIRMA_LISTADO), tid, dir_a_topic(&vendedor)], u128_a_u256(precio));
    }

    pub fn venta(&mut self, market: Direccion, id: u64, comprador: Direccion, precio: u128) {
        let mut tid = [0u8; 32];
        tid[24..].copy_from_slice(&id.to_be_bytes());
        self.emitir(market, vec![topic_de(FIRMA_VENTA), tid, dir_a_topic(&comprador)], u128_a_u256(precio));
    }

    /// Cierra un bloque con los logs pendientes.
    pub fn minar(&mut self) -> u64 {
        let numero = self.bloques.len() as u64;
        let padre = self.bloques.last().map(|b| b.hash).unwrap_or([0; 32]);
        let mut h = Keccak256::new();
        h.update(padre);
        h.update(numero.to_be_bytes());
        h.update([self.rama]);
        for (c, t, d) in &self.pendientes {
            h.update(c);
            t.iter().for_each(|x| h.update(x));
            h.update(d);
        }
        let hash: H256 = h.finalize().into();
        let logs = self
            .pendientes
            .drain(..)
            .enumerate()
            .map(|(i, (contrato, topics, data))| Log { contrato, topics, data, bloque: numero, hash_bloque: hash, indice_tx: i as u32, indice_log: i as u32 })
            .collect();
        self.bloques.push(BloqueConLogs { numero, hash, padre, logs });
        numero
    }

    /// Simula una reorganización: descarta los últimos `profundidad` bloques
    /// (la rama nueva se mina después con `minar`, con otro hash).
    pub fn reorg(&mut self, profundidad: usize) {
        let n = self.bloques.len().saturating_sub(profundidad).max(1);
        self.bloques.truncate(n);
        self.rama = self.rama.wrapping_add(1);
    }
}

impl FuenteBloques for CadenaMock {
    fn ultimo(&self) -> u64 {
        self.bloques.len() as u64 - 1
    }
    fn bloque(&self, n: u64) -> Option<BloqueConLogs> {
        self.bloques.get(n as usize).cloned()
    }
}
