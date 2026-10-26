//! # Cliente IMAP con reglas
//!
//! - [`reglas`]: motor de reglas puro (qué hacer con cada mensaje).
//! - [`imap`]: cliente IMAP mínimo genérico sobre `Read + Write`.
//! - [`procesar`]: orquesta ambos: busca mensajes, evalúa reglas y aplica acciones
//!   (o solo informa en modo simulación).
//! - [`servidor_simulado`]: servidor IMAP en memoria (subconjunto del protocolo)
//!   usado en los tests y en `--demo`, para probar todo **sin un servidor real**.

pub mod imap;
pub mod reglas;
pub mod servidor_simulado;

use imap::{Cliente, ErrorImap};
use reglas::{evaluar, Accion, Regla};
use std::io::{Read, Write};

/// Resultado de procesar un mensaje.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aplicada {
    pub uid: u32,
    pub asunto: String,
    pub regla: String,
    pub accion: Accion,
}

/// Procesa la carpeta seleccionada aplicando reglas a los mensajes que cumplan `criterio`.
pub fn procesar<S: Read + Write>(
    c: &mut Cliente<S>,
    reglas: &[Regla],
    criterio: &str,
    simular: bool,
) -> Result<Vec<Aplicada>, ErrorImap> {
    let uids = c.buscar(criterio)?;
    let mensajes = c.cabeceras(&uids)?;
    let mut hechas = Vec::new();
    let mut hubo_borrados = false;
    for m in &mensajes {
        let Some(r) = evaluar(reglas, m) else { continue };
        if !simular {
            match &r.accion {
                Accion::Mover(dest) => {
                    c.mover(m.uid, dest)?;
                    hubo_borrados = true;
                }
                Accion::MarcarLeido => c.agregar_bandera(m.uid, "\\Seen")?,
                Accion::Destacar => c.agregar_bandera(m.uid, "\\Flagged")?,
                Accion::Borrar => {
                    c.agregar_bandera(m.uid, "\\Deleted")?;
                    hubo_borrados = true;
                }
            }
        }
        hechas.push(Aplicada { uid: m.uid, asunto: m.asunto.clone(), regla: r.nombre.clone(), accion: r.accion.clone() });
    }
    if hubo_borrados {
        c.expunge()?;
    }
    Ok(hechas)
}
