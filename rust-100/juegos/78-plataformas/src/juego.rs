//! Reglas: monedas, pinchos, caídas, meta, vidas y cámara.

use crate::fisica::{Cuerpo, Entrada};
use crate::nivel::{Nivel, Tile};

pub const DT: f32 = 1.0 / 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    Jugando,
    Gano,
    Perdio,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evento {
    Moneda,
    Murio,
    Gano,
}

pub struct Juego {
    pub nivel: Nivel,
    pub jugador: Cuerpo,
    pub monedas: u32,
    pub vidas: u32,
    pub estado: Estado,
    pub tiempo: f32,
}

impl Juego {
    pub fn new(nivel: Nivel) -> Self {
        let (x, y) = nivel.inicio;
        Self { jugador: Cuerpo::new(x, y), nivel, monedas: 0, vidas: 3, estado: Estado::Jugando, tiempo: 0.0 }
    }

    pub fn actualizar(&mut self, e: Entrada) -> Vec<Evento> {
        let mut ev = vec![];
        if self.estado != Estado::Jugando {
            return ev;
        }
        self.tiempo += DT;
        self.jugador.actualizar(&self.nivel, e, DT);
        let mut muere = self.jugador.y > self.nivel.alto as f32 + 1.0;
        for (x, y) in self.jugador.tocando() {
            match self.nivel.tile(x, y) {
                Tile::Moneda => {
                    self.nivel.poner(x, y, Tile::Vacio);
                    self.monedas += 1;
                    ev.push(Evento::Moneda);
                }
                Tile::Pinchos => muere = true,
                Tile::Meta => {
                    self.estado = Estado::Gano;
                    ev.push(Evento::Gano);
                    return ev;
                }
                _ => {}
            }
        }
        if muere {
            self.vidas -= 1;
            ev.push(Evento::Murio);
            if self.vidas == 0 {
                self.estado = Estado::Perdio;
            } else {
                let (x, y) = self.nivel.inicio;
                self.jugador = Cuerpo::new(x, y);
            }
        }
        ev
    }

    /// Cámara: centra al jugador en X sin salirse del nivel.
    pub fn camara_x(&self, ancho_vista: usize) -> usize {
        let centro = self.jugador.x as i32 - ancho_vista as i32 / 2;
        centro.clamp(0, (self.nivel.ancho as i32 - ancho_vista as i32).max(0)) as usize
    }

    pub fn renderizar(&self, ancho_vista: usize) -> String {
        let cx = self.camara_x(ancho_vista);
        let (px, py) = ((self.jugador.x + 0.4) as i32, (self.jugador.y + 0.45) as i32);
        let mut s = String::new();
        for y in 0..self.nivel.alto as i32 {
            for x in cx as i32..(cx + ancho_vista).min(self.nivel.ancho) as i32 {
                s.push(if (x, y) == (px, py) {
                    '@'
                } else {
                    match self.nivel.tile(x, y) {
                        Tile::Vacio => ' ',
                        Tile::Solido => '#',
                        Tile::UnSentido => '=',
                        Tile::Pinchos => '^',
                        Tile::Moneda => 'o',
                        Tile::Meta => 'F',
                    }
                });
            }
            s.push('\n');
        }
        s + &format!("Monedas {}  Vidas {}  t={:.1}s", self.monedas, self.vidas, self.tiempo)
    }
}
