//! El indexador: ingiere bloques, decodifica logs y mantiene índices.
//!
//! **Reorgs**: guardamos el hash de cada bloque indexado. Antes de procesar el
//! bloque N verificamos que su `padre` coincida con nuestro hash de N-1; si no,
//! retrocedemos (borrando eventos de los bloques huérfanos) hasta encontrar el
//! ancestro común. Además sólo consideramos "finales" los bloques con
//! `confirmaciones` de profundidad.

use crate::cadena_mock::FuenteBloques;
use crate::modelo::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;

/// Evento indexado: log crudo + versión decodificada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registro {
    pub id: u64,
    pub log: Log,
    pub evento: Evento,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErrorIndexador {
    #[error("reorg más profundo que la ventana guardada")]
    ReorgProfundo,
    #[error("checkpoint corrupto: {0}")]
    Checkpoint(String),
}

/// Estado persistible (checkpoint) del indexador.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Indexador {
    /// Contratos que nos interesan (vacío = todos).
    pub contratos: Vec<Direccion>,
    /// Último bloque procesado.
    pub cursor: Option<u64>,
    /// Hashes de bloques procesados (para detectar reorgs).
    pub hashes: BTreeMap<u64, H256>,
    pub registros: BTreeMap<u64, Registro>,
    pub siguiente_id: u64,
    /// Cantidad de reorgs detectados (métrica).
    pub reorgs: u64,
    #[serde(skip)]
    por_direccion: HashMap<Direccion, Vec<u64>>,
}

/// Filtro de consulta (similar a `eth_getLogs`).
#[derive(Debug, Clone, Default)]
pub struct Filtro {
    pub desde: Option<u64>,
    pub hasta: Option<u64>,
    pub contrato: Option<Direccion>,
    pub topic0: Option<H256>,
    /// Dirección involucrada en cualquier topic indexado.
    pub participante: Option<Direccion>,
}

/// Página de resultados con cursor para la siguiente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pagina {
    pub items: Vec<Registro>,
    pub siguiente: Option<u64>,
}

impl Indexador {
    pub fn nuevo(contratos: Vec<Direccion>) -> Self {
        Self { contratos, ..Default::default() }
    }

    /// Reconstruye índices secundarios (tras cargar un checkpoint).
    fn reindexar(&mut self) {
        self.por_direccion.clear();
        let pares: Vec<(u64, Vec<Direccion>)> = self.registros.values().map(|r| (r.id, participantes(&r.log))).collect();
        for (id, ds) in pares {
            for d in ds {
                self.por_direccion.entry(d).or_default().push(id);
            }
        }
    }

    fn agregar(&mut self, log: Log) {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        for d in participantes(&log) {
            self.por_direccion.entry(d).or_default().push(id);
        }
        let evento = decodificar(&log);
        self.registros.insert(id, Registro { id, log, evento });
    }

    /// Borra todo lo indexado a partir del bloque `desde` (inclusive).
    fn revertir_desde(&mut self, desde: u64) {
        self.registros.retain(|_, r| r.log.bloque < desde);
        self.hashes.retain(|b, _| *b < desde);
        self.cursor = desde.checked_sub(1);
        self.reindexar();
    }

    /// Procesa hasta `ultimo - confirmaciones`. Devuelve bloques procesados.
    pub fn sincronizar<F: FuenteBloques>(&mut self, fuente: &F, confirmaciones: u64) -> Result<u64, ErrorIndexador> {
        let objetivo = fuente.ultimo().saturating_sub(confirmaciones);
        let mut procesados = 0;
        loop {
            // 1) ¿Nuestra punta sigue en la cadena canónica? Si el bloque del
            //    cursor cambió de hash (o desapareció), retrocedemos.
            if let Some(c) = self.cursor {
                let canonico = fuente.bloque(c).map(|b| b.hash);
                if canonico.as_ref() != self.hashes.get(&c) {
                    self.reorgs += 1;
                    if c == 0 || !self.hashes.contains_key(&(c - 1)) {
                        return Err(ErrorIndexador::ReorgProfundo);
                    }
                    self.revertir_desde(c);
                    continue;
                }
            }
            let n = self.cursor.map(|c| c + 1).unwrap_or(0);
            if n > objetivo {
                break;
            }
            let Some(b) = fuente.bloque(n) else { break };
            // ¿El padre coincide con lo que indexamos? Si no → reorg.
            if n > 0 {
                if let Some(h) = self.hashes.get(&(n - 1)) {
                    if *h != b.padre {
                        self.reorgs += 1;
                        // Retrocedemos un bloque y reintentamos.
                        let primero = *self.hashes.keys().next().unwrap_or(&0);
                        if n - 1 < primero || n - 1 == 0 {
                            return Err(ErrorIndexador::ReorgProfundo);
                        }
                        self.revertir_desde(n - 1);
                        continue;
                    }
                }
            }
            for log in b.logs {
                if self.contratos.is_empty() || self.contratos.contains(&log.contrato) {
                    self.agregar(log);
                }
            }
            self.hashes.insert(n, b.hash);
            self.cursor = Some(n);
            procesados += 1;
        }
        // Podamos hashes viejos (ventana de 256 bloques).
        if let Some(c) = self.cursor {
            let corte = c.saturating_sub(256);
            self.hashes.retain(|b, _| *b >= corte);
        }
        Ok(procesados)
    }

    /// Consulta paginada por id creciente.
    pub fn consultar(&self, f: &Filtro, desde_id: u64, limite: usize) -> Pagina {
        let candidatos: Box<dyn Iterator<Item = &Registro>> = match f.participante {
            // Índice secundario por dirección: evitamos escanear todo.
            Some(d) => Box::new(self.por_direccion.get(&d).into_iter().flatten().filter_map(|id| self.registros.get(id))),
            None => Box::new(self.registros.range(desde_id..).map(|(_, r)| r)),
        };
        let mut items: Vec<Registro> = candidatos
            .filter(|r| r.id >= desde_id)
            .filter(|r| f.desde.is_none_or(|d| r.log.bloque >= d))
            .filter(|r| f.hasta.is_none_or(|h| r.log.bloque <= h))
            .filter(|r| f.contrato.is_none_or(|c| r.log.contrato == c))
            .filter(|r| f.topic0.is_none_or(|t| r.log.topics.first() == Some(&t)))
            .take(limite + 1)
            .cloned()
            .collect();
        let siguiente = if items.len() > limite { items.pop().map(|r| r.id) } else { None };
        Pagina { items, siguiente }
    }

    /// Agregado: saldos de un token a partir de sus `Transfer` (la dirección 0 = mint/burn).
    pub fn saldos(&self, token: &Direccion) -> BTreeMap<Direccion, i128> {
        let mut s = BTreeMap::new();
        for r in self.registros.values().filter(|r| r.log.contrato == *token) {
            if let Evento::Transfer { de, para, valor } = r.evento {
                if de != [0; 20] {
                    *s.entry(de).or_insert(0) -= valor as i128;
                }
                if para != [0; 20] {
                    *s.entry(para).or_insert(0) += valor as i128;
                }
            }
        }
        s
    }

    /// Agregado del marketplace: listados activos (sin vender).
    pub fn listados_activos(&self) -> Vec<(u64, Direccion, u128)> {
        let mut activos: BTreeMap<u64, (Direccion, u128)> = BTreeMap::new();
        for r in self.registros.values() {
            match r.evento {
                Evento::Listado { id, vendedor, precio } => {
                    activos.insert(id, (vendedor, precio));
                }
                Evento::Venta { id, .. } => {
                    activos.remove(&id);
                }
                _ => {}
            }
        }
        activos.into_iter().map(|(id, (v, p))| (id, v, p)).collect()
    }

    pub fn guardar_checkpoint(&self) -> String {
        serde_json::to_string(self).expect("serializable")
    }

    pub fn cargar_checkpoint(s: &str) -> Result<Self, ErrorIndexador> {
        let mut i: Self = serde_json::from_str(s).map_err(|e| ErrorIndexador::Checkpoint(e.to_string()))?;
        i.reindexar();
        Ok(i)
    }
}

/// Direcciones presentes en los topics indexados (heurística: 12 bytes de ceros).
fn participantes(log: &Log) -> Vec<Direccion> {
    log.topics.iter().skip(1).filter(|t| t[..12].iter().all(|b| *b == 0) && t[12..].iter().any(|b| *b != 0)).map(topic_a_dir).collect()
}
