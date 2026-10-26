//! Algoritmo **token bucket** (balde de fichas), sin I/O ni reloj propio.
//!
//! Cada clave tiene un balde con capacidad `rafaga` que se rellena a
//! `por_segundo` fichas/s. Cada pedido consume 1 ficha; sin fichas → rechazo.
//! Permite ráfagas cortas pero limita el promedio. Las fichas se calculan
//! "perezosamente" al consultar (no hace falta un timer por clave).

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    /// Fichas máximas (tamaño de ráfaga).
    pub rafaga: f64,
    /// Ritmo de recarga (fichas por segundo).
    pub por_segundo: f64,
    /// Máximo de claves en memoria (defensa contra agotar RAM con IPs falsas).
    pub max_claves: usize,
}

#[derive(Debug, Clone, Copy)]
struct Balde {
    fichas: f64,
    ultimo_ms: u64,
}

/// Resultado de una consulta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decision {
    /// Permitido; quedan `restantes` fichas enteras.
    Permitido { restantes: u64 },
    /// Rechazado; reintentar en `reintentar_ms`.
    Limitado { reintentar_ms: u64 },
    /// No hay lugar para una clave nueva (tabla llena de clientes activos).
    Saturado,
}

#[derive(Debug)]
pub struct Baldes {
    cfg: Config,
    mapa: HashMap<String, Balde>,
}

impl Baldes {
    pub fn new(cfg: Config) -> Self {
        assert!(cfg.rafaga >= 1.0 && cfg.por_segundo > 0.0, "configuración inválida");
        Self { cfg, mapa: HashMap::new() }
    }

    pub fn config(&self) -> Config {
        self.cfg
    }

    pub fn claves(&self) -> usize {
        self.mapa.len()
    }

    fn recargar(cfg: &Config, b: &mut Balde, ahora_ms: u64) {
        let dt = ahora_ms.saturating_sub(b.ultimo_ms) as f64 / 1000.0;
        b.fichas = (b.fichas + dt * cfg.por_segundo).min(cfg.rafaga);
        b.ultimo_ms = b.ultimo_ms.max(ahora_ms); // el reloj nunca "retrocede"
    }

    /// Elimina baldes LLENOS: un balde lleno equivale a uno nuevo, así que
    /// borrarlo no cambia el comportamiento y libera memoria.
    pub fn purgar(&mut self, ahora_ms: u64) {
        let cfg = self.cfg;
        self.mapa.retain(|_, b| {
            Self::recargar(&cfg, b, ahora_ms);
            b.fichas < cfg.rafaga
        });
    }

    /// Intenta consumir una ficha para `clave`.
    pub fn intentar(&mut self, clave: &str, ahora_ms: u64) -> Decision {
        if !self.mapa.contains_key(clave) && self.mapa.len() >= self.cfg.max_claves {
            self.purgar(ahora_ms);
            if self.mapa.len() >= self.cfg.max_claves {
                return Decision::Saturado;
            }
        }
        let cfg = self.cfg;
        let b = self.mapa.entry(clave.to_string()).or_insert(Balde { fichas: cfg.rafaga, ultimo_ms: ahora_ms });
        Self::recargar(&cfg, b, ahora_ms);
        if b.fichas >= 1.0 {
            b.fichas -= 1.0;
            Decision::Permitido { restantes: b.fichas.floor() as u64 }
        } else {
            let falta = 1.0 - b.fichas;
            Decision::Limitado { reintentar_ms: (falta / cfg.por_segundo * 1000.0).ceil() as u64 }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(r: f64, s: f64, m: usize) -> Config {
        Config { rafaga: r, por_segundo: s, max_claves: m }
    }

    #[test]
    fn rafaga_recarga_y_espera() {
        let mut b = Baldes::new(cfg(3.0, 2.0, 100));
        assert_eq!(b.intentar("a", 0), Decision::Permitido { restantes: 2 });
        assert_eq!(b.intentar("a", 0), Decision::Permitido { restantes: 1 });
        assert_eq!(b.intentar("a", 0), Decision::Permitido { restantes: 0 });
        // 2 fichas/s → falta 1 ficha → 500 ms.
        assert_eq!(b.intentar("a", 0), Decision::Limitado { reintentar_ms: 500 });
        assert_eq!(b.intentar("a", 250), Decision::Limitado { reintentar_ms: 250 });
        assert_eq!(b.intentar("a", 500), Decision::Permitido { restantes: 0 });
        // Otra clave es independiente.
        assert_eq!(b.intentar("b", 500), Decision::Permitido { restantes: 2 });
        // Mucho tiempo después no supera la ráfaga.
        assert_eq!(b.intentar("a", 1_000_000), Decision::Permitido { restantes: 2 });
        // Reloj que retrocede: no regala fichas.
        let mut c = Baldes::new(cfg(1.0, 1.0, 10));
        c.intentar("x", 5000);
        assert!(matches!(c.intentar("x", 1000), Decision::Limitado { .. }));
    }

    #[test]
    fn limite_de_claves_y_purga() {
        let mut b = Baldes::new(cfg(2.0, 1.0, 2));
        b.intentar("a", 0);
        b.intentar("b", 0);
        // Tabla llena con clientes activos → saturado.
        assert_eq!(b.intentar("c", 0), Decision::Saturado);
        // Pasado 1 s los baldes se llenaron → se purgan y entra "c".
        assert_eq!(b.intentar("c", 1000), Decision::Permitido { restantes: 1 });
        assert_eq!(b.claves(), 1);
    }
}
