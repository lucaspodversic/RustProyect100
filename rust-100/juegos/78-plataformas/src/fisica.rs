//! Física del jugador y resolución de colisiones contra el tilemap.
//!
//! Técnica: movemos **primero en X y después en Y**, y tras cada eje
//! empujamos al jugador fuera de los tiles sólidos que solape. Separar ejes
//! evita el clásico bug de "engancharse" en esquinas. Para que no atraviese
//! paredes a alta velocidad (tunneling), subdividimos el movimiento en pasos
//! de como máximo media celda.

use crate::nivel::{Nivel, Tile};

pub const GRAVEDAD: f32 = 60.0; // celdas/s²
pub const VEL_MAX_CAIDA: f32 = 30.0;
pub const VEL_CORRER: f32 = 9.0;
pub const ACELERACION: f32 = 70.0;
pub const FRICCION: f32 = 50.0;
pub const VEL_SALTO: f32 = 21.0;
/// Tiempo de gracia para saltar después de dejar una plataforma.
pub const COYOTE: f32 = 0.1;
/// Si apretás saltar un poco antes de tocar el piso, igual salta.
pub const BUFFER_SALTO: f32 = 0.12;
pub const ANCHO_J: f32 = 0.8;
pub const ALTO_J: f32 = 0.9;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Entrada {
    pub izq: bool,
    pub der: bool,
    pub saltar: bool,
    /// Mantener abajo + saltar atraviesa plataformas de un sentido.
    pub abajo: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cuerpo {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub en_suelo: bool,
    pub tiempo_coyote: f32,
    pub tiempo_buffer: f32,
    saltar_previo: bool,
    /// Ignorar plataformas de un sentido por un rato (bajar a través).
    pub atravesar: f32,
}

impl Cuerpo {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x: x + (1.0 - ANCHO_J) / 2.0, y: y + (1.0 - ALTO_J), vx: 0.0, vy: 0.0, en_suelo: false, tiempo_coyote: 0.0, tiempo_buffer: 0.0, saltar_previo: false, atravesar: 0.0 }
    }

    /// Celdas que solapa la caja del cuerpo.
    fn celdas(&self) -> impl Iterator<Item = (i32, i32)> {
        let (x0, x1) = (self.x.floor() as i32, (self.x + ANCHO_J - 1e-4).floor() as i32);
        let (y0, y1) = (self.y.floor() as i32, (self.y + ALTO_J - 1e-4).floor() as i32);
        (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| (x, y)))
    }

    pub fn actualizar(&mut self, n: &Nivel, e: Entrada, dt: f32) {
        // --- Horizontal: aceleración y fricción (control con inercia).
        let dir = e.der as i32 as f32 - e.izq as i32 as f32;
        if dir != 0.0 {
            self.vx = (self.vx + dir * ACELERACION * dt).clamp(-VEL_CORRER, VEL_CORRER);
        } else {
            let f = FRICCION * dt;
            self.vx = if self.vx.abs() <= f { 0.0 } else { self.vx - f * self.vx.signum() };
        }

        // --- Salto: buffer + coyote + salto variable.
        let flanco = e.saltar && !self.saltar_previo;
        self.saltar_previo = e.saltar;
        if flanco {
            self.tiempo_buffer = BUFFER_SALTO;
        } else {
            self.tiempo_buffer = (self.tiempo_buffer - dt).max(0.0);
        }
        self.tiempo_coyote = if self.en_suelo { COYOTE } else { (self.tiempo_coyote - dt).max(0.0) };
        self.atravesar = (self.atravesar - dt).max(0.0);
        if self.tiempo_buffer > 0.0 && self.tiempo_coyote > 0.0 {
            if e.abajo && self.sobre_un_sentido(n) {
                self.atravesar = 0.2;
            } else {
                self.vy = -VEL_SALTO;
            }
            self.tiempo_buffer = 0.0;
            self.tiempo_coyote = 0.0;
        }
        // Soltar el botón corta el salto (salto corto vs. largo).
        if !e.saltar && self.vy < -VEL_SALTO / 3.0 {
            self.vy = -VEL_SALTO / 3.0;
        }
        self.vy = (self.vy + GRAVEDAD * dt).min(VEL_MAX_CAIDA);

        // --- Integración con sub-pasos anti-tunneling.
        let dx = self.vx * dt;
        let dy = self.vy * dt;
        let pasos = ((dx.abs().max(dy.abs())) / 0.45).ceil().max(1.0) as i32;
        self.en_suelo = false;
        for _ in 0..pasos {
            self.mover_x(n, dx / pasos as f32);
            self.mover_y(n, dy / pasos as f32);
        }
    }

    fn sobre_un_sentido(&self, n: &Nivel) -> bool {
        let y = (self.y + ALTO_J + 0.01).floor() as i32;
        let (x0, x1) = (self.x.floor() as i32, (self.x + ANCHO_J - 1e-4).floor() as i32);
        (x0..=x1).all(|x| matches!(n.tile(x, y), Tile::UnSentido | Tile::Vacio)) && (x0..=x1).any(|x| n.tile(x, y) == Tile::UnSentido)
    }

    fn mover_x(&mut self, n: &Nivel, dx: f32) {
        self.x += dx;
        for (cx, cy) in self.celdas().collect::<Vec<_>>() {
            if n.tile(cx, cy) == Tile::Solido {
                if dx > 0.0 {
                    self.x = cx as f32 - ANCHO_J;
                } else if dx < 0.0 {
                    self.x = cx as f32 + 1.0;
                }
                self.vx = 0.0;
            }
        }
    }

    fn mover_y(&mut self, n: &Nivel, dy: f32) {
        let pie_antes = self.y + ALTO_J;
        self.y += dy;
        for (cx, cy) in self.celdas().collect::<Vec<_>>() {
            let t = n.tile(cx, cy);
            // Un-sentido: sólo bloquea si veníamos cayendo desde ARRIBA de ella.
            let bloquea = t == Tile::Solido || (t == Tile::UnSentido && dy > 0.0 && pie_antes <= cy as f32 + 1e-3 && self.atravesar == 0.0);
            if bloquea {
                if dy > 0.0 {
                    self.y = cy as f32 - ALTO_J;
                    self.en_suelo = true;
                } else if dy < 0.0 {
                    self.y = cy as f32 + 1.0; // cabezazo contra el techo
                }
                self.vy = 0.0;
            }
        }
    }

    /// Tiles que toca actualmente (para monedas/pinchos/meta).
    pub fn tocando(&self) -> Vec<(i32, i32)> {
        self.celdas().collect()
    }
}
