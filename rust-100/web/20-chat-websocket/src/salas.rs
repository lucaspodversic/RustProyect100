//! Lógica de salas, independiente de WebSockets (testeable sin red).
//!
//! Cada sala tiene:
//! - un canal `tokio::sync::broadcast`: todos los suscriptores reciben cada mensaje;
//! - un **historial** acotado (`VecDeque` con capacidad máxima) que se envía
//!   a quien se une, para que vea la conversación reciente.
//!
//! Las salas se crean bajo demanda y se eliminan cuando queda vacía
//! (sin suscriptores), para no acumular memoria.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

/// Mensaje que viaja por el chat (JSON).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Evento {
    Mensaje { de: String, texto: String, ts: u64 },
    Entro { nombre: String, ts: u64 },
    Salio { nombre: String, ts: u64 },
    /// Lista de mensajes previos enviada solo al que se une.
    Historial { mensajes: Vec<Evento> },
    Error { motivo: String },
}

pub fn ahora_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

struct Sala {
    tx: broadcast::Sender<Evento>,
    historial: VecDeque<Evento>,
    miembros: Vec<String>,
}

/// Registro de salas compartido entre conexiones.
#[derive(Clone)]
pub struct Salas {
    inner: Arc<Mutex<HashMap<String, Sala>>>,
    pub max_historial: usize,
}

/// Lo que recibe una conexión al unirse.
pub struct Union {
    pub rx: broadcast::Receiver<Evento>,
    pub historial: Vec<Evento>,
}

impl Salas {
    pub fn new(max_historial: usize) -> Self {
        Salas { inner: Arc::default(), max_historial }
    }

    /// Une a `nombre` a `sala`. Falla si el nombre ya está en uso en esa sala.
    pub fn unir(&self, sala: &str, nombre: &str) -> Result<Union, String> {
        let mut m = self.inner.lock().expect("mutex");
        let s = m.entry(sala.to_string()).or_insert_with(|| Sala {
            tx: broadcast::channel(128).0, // buffer por suscriptor; los lentos pierden mensajes viejos
            historial: VecDeque::new(),
            miembros: vec![],
        });
        if s.miembros.iter().any(|x| x.eq_ignore_ascii_case(nombre)) {
            return Err(format!("el nombre '{nombre}' ya está en uso en la sala"));
        }
        s.miembros.push(nombre.to_string());
        // Suscribirse ANTES de anunciar, así el propio usuario ve su "entró".
        let rx = s.tx.subscribe();
        let historial = s.historial.iter().cloned().collect();
        let _ = s.tx.send(Evento::Entro { nombre: nombre.into(), ts: ahora_ms() });
        Ok(Union { rx, historial })
    }

    /// Publica un mensaje (lo guarda en el historial).
    pub fn publicar(&self, sala: &str, de: &str, texto: &str) {
        let mut m = self.inner.lock().expect("mutex");
        if let Some(s) = m.get_mut(sala) {
            let e = Evento::Mensaje { de: de.into(), texto: texto.into(), ts: ahora_ms() };
            s.historial.push_back(e.clone());
            while s.historial.len() > self.max_historial {
                s.historial.pop_front();
            }
            // Err = sin receptores: no es un problema.
            let _ = s.tx.send(e);
        }
    }

    /// Saca al usuario; borra la sala si queda vacía.
    pub fn salir(&self, sala: &str, nombre: &str) {
        let mut m = self.inner.lock().expect("mutex");
        if let Some(s) = m.get_mut(sala) {
            s.miembros.retain(|x| x != nombre);
            let _ = s.tx.send(Evento::Salio { nombre: nombre.into(), ts: ahora_ms() });
            if s.miembros.is_empty() {
                m.remove(sala);
            }
        }
    }

    /// Salas activas con cantidad de miembros.
    pub fn listar(&self) -> Vec<(String, usize)> {
        let m = self.inner.lock().expect("mutex");
        let mut v: Vec<_> = m.iter().map(|(k, s)| (k.clone(), s.miembros.len())).collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn historial_acotado_y_difusion() {
        let s = Salas::new(2);
        let mut a = s.unir("rust", "ana").unwrap();
        for t in ["1", "2", "3"] {
            s.publicar("rust", "ana", t);
        }
        let b = s.unir("rust", "beto").unwrap();
        let textos: Vec<_> = b.historial.iter().map(|e| match e { Evento::Mensaje { texto, .. } => texto.as_str(), _ => "" }).collect();
        assert_eq!(textos, vec!["2", "3"]);
        // ana recibe: su Entro, 3 mensajes, Entro de beto.
        let mut n = 0;
        while a.rx.try_recv().is_ok() {
            n += 1;
        }
        assert_eq!(n, 5);
    }

    #[test]
    fn nombres_unicos_y_limpieza() {
        let s = Salas::new(10);
        let _a = s.unir("x", "Ana").unwrap();
        assert!(s.unir("x", "ana").is_err());
        assert!(s.unir("y", "ana").is_ok()); // otra sala: ok
        assert_eq!(s.listar(), vec![("x".into(), 1), ("y".into(), 1)]);
        s.salir("x", "Ana");
        assert_eq!(s.listar(), vec![("y".into(), 1)]);
    }
}
