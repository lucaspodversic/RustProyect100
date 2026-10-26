//! Protocolo de texto, una línea por mensaje (fácil de depurar con `nc`).
//!
//! Cliente → servidor:  `HOLA <nombre>` · `IN <-1|0|1>` · `CHAU`
//! Servidor → cliente:  `BIENVENIDO <jugador 0|1>` · `INICIO <rival>` ·
//!   `ESTADO <tick> <p0> <p1> <bx> <by> <vx> <vy> <pts0> <pts1>` ·
//!   `FIN <ganador|->` · `ERROR <motivo>`

use crate::juego::Estado;
use thiserror::Error;

pub const MAX_LINEA: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub enum MsgCliente {
    Hola(String),
    Entrada(i8),
    Chau,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MsgServidor {
    Bienvenido(usize),
    Inicio(String),
    Estado(Estado),
    Fin(Option<usize>),
    Error(String),
}

#[derive(Debug, Error, PartialEq)]
pub enum ErrorProtocolo {
    #[error("línea demasiado larga")]
    Larga,
    #[error("mensaje inválido: {0}")]
    Invalido(String),
}

/// Nombres: alfanuméricos y `_-`, máx. 16 caracteres (sin espacios ni control).
pub fn sanear_nombre(n: &str) -> String {
    let s: String = n.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-').take(16).collect();
    if s.is_empty() { "anon".into() } else { s }
}

impl MsgCliente {
    pub fn a_linea(&self) -> String {
        match self {
            MsgCliente::Hola(n) => format!("HOLA {}\n", sanear_nombre(n)),
            MsgCliente::Entrada(d) => format!("IN {d}\n"),
            MsgCliente::Chau => "CHAU\n".into(),
        }
    }

    pub fn parsear(l: &str) -> Result<Self, ErrorProtocolo> {
        if l.len() > MAX_LINEA {
            return Err(ErrorProtocolo::Larga);
        }
        let mut p = l.split_whitespace();
        let inv = || ErrorProtocolo::Invalido(l.trim().chars().take(40).collect());
        match p.next() {
            Some("HOLA") => Ok(MsgCliente::Hola(sanear_nombre(p.next().unwrap_or("")))),
            Some("IN") => match p.next().and_then(|x| x.parse::<i8>().ok()) {
                Some(d @ -1..=1) => Ok(MsgCliente::Entrada(d)),
                _ => Err(inv()),
            },
            Some("CHAU") => Ok(MsgCliente::Chau),
            _ => Err(inv()),
        }
    }
}

impl MsgServidor {
    pub fn a_linea(&self) -> String {
        match self {
            MsgServidor::Bienvenido(j) => format!("BIENVENIDO {j}\n"),
            MsgServidor::Inicio(r) => format!("INICIO {r}\n"),
            MsgServidor::Estado(e) => format!(
                "ESTADO {} {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} {} {}\n",
                e.tick, e.paletas[0], e.paletas[1], e.pelota.0, e.pelota.1, e.vel.0, e.vel.1, e.puntos[0], e.puntos[1]
            ),
            MsgServidor::Fin(g) => format!("FIN {}\n", g.map(|g| g.to_string()).unwrap_or("-".into())),
            MsgServidor::Error(m) => format!("ERROR {m}\n"),
        }
    }

    pub fn parsear(l: &str) -> Result<Self, ErrorProtocolo> {
        if l.len() > MAX_LINEA {
            return Err(ErrorProtocolo::Larga);
        }
        let inv = || ErrorProtocolo::Invalido(l.trim().chars().take(40).collect());
        let (cab, resto) = l.trim().split_once(' ').unwrap_or((l.trim(), ""));
        match cab {
            "BIENVENIDO" => Ok(MsgServidor::Bienvenido(resto.parse().ok().filter(|j| *j < 2).ok_or_else(inv)?)),
            "INICIO" => Ok(MsgServidor::Inicio(resto.into())),
            "FIN" => Ok(MsgServidor::Fin(if resto == "-" { None } else { Some(resto.parse().map_err(|_| inv())?) })),
            "ERROR" => Ok(MsgServidor::Error(resto.into())),
            "ESTADO" => {
                let c: Vec<&str> = resto.split(' ').collect();
                if c.len() != 9 {
                    return Err(inv());
                }
                let f = |i: usize| c[i].parse::<f32>().ok().filter(|v| v.is_finite()).ok_or_else(inv);
                let u = |i: usize| c[i].parse::<u64>().map_err(|_| inv());
                Ok(MsgServidor::Estado(Estado {
                    tick: u(0)?,
                    paletas: [f(1)?, f(2)?],
                    pelota: (f(3)?, f(4)?),
                    vel: (f(5)?, f(6)?),
                    puntos: [u(7)? as u32, u(8)? as u32],
                    ganador: None,
                }))
            }
            _ => Err(inv()),
        }
    }
}
