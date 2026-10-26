//! Lógica del shooter: nave del jugador abajo, enemigos que bajan en oleadas
//! y disparan; balas; colisiones; vidas, invulnerabilidad temporal y puntaje.
//!
//! Se actualiza con **paso fijo** (`DT`) para que la simulación sea
//! determinista: misma semilla + mismas entradas ⇒ misma partida (replays).

use crate::motor::*;

pub const DT: f32 = 1.0 / 30.0;
pub const ANCHO: f32 = 40.0;
pub const ALTO: f32 = 24.0;

/// Entrada abstracta (teclado, gamepad, IA o test la producen igual).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Entrada {
    pub izquierda: bool,
    pub derecha: bool,
    pub disparar: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bala {
    pub caja: Aabb,
    pub vel: V2,
    pub del_jugador: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Enemigo {
    pub caja: Aabb,
    pub vel: V2,
    pub vida: u8,
    pub puntos: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventoJuego {
    EnemigoDestruido { puntos: u32 },
    JugadorHerido { vidas: u8 },
    NuevaOleada(u32),
    FinDelJuego,
}

#[derive(Debug, Clone)]
pub struct Juego {
    pub jugador: Aabb,
    pub balas: Vec<Bala>,
    pub enemigos: Vec<Enemigo>,
    pub puntaje: u32,
    pub vidas: u8,
    pub oleada: u32,
    pub terminado: bool,
    /// Segundos de invulnerabilidad tras recibir daño.
    pub invulnerable: f32,
    /// Enfriamiento del arma.
    pub recarga: f32,
    pub tiempo: f32,
    pub rng: Rng,
}

pub const VEL_JUGADOR: f32 = 20.0;
pub const VEL_BALA: f32 = 30.0;
pub const RECARGA: f32 = 0.25;

impl Juego {
    pub fn new(semilla: u64) -> Self {
        Self {
            jugador: Aabb { pos: V2::new(ANCHO / 2.0 - 1.5, ALTO - 2.0), tam: V2::new(3.0, 1.0) },
            balas: vec![],
            enemigos: vec![],
            puntaje: 0,
            vidas: 3,
            oleada: 0,
            terminado: false,
            invulnerable: 0.0,
            recarga: 0.0,
            tiempo: 0.0,
            rng: Rng::new(semilla),
        }
    }

    /// Genera una oleada: más enemigos y más rápidos cada vez.
    fn generar_oleada(&mut self) {
        self.oleada += 1;
        let n = 3 + self.oleada as usize * 2;
        for i in 0..n {
            let x = 2.0 + (i as f32 * 5.0) % (ANCHO - 6.0);
            let y = 1.0 + (i as f32 * 5.0 / (ANCHO - 6.0)).floor() * 2.0;
            let rapido = self.rng.f32() < 0.2;
            self.enemigos.push(Enemigo {
                caja: Aabb { pos: V2::new(x, y), tam: V2::new(2.0, 1.0) },
                vel: V2::new(if i % 2 == 0 { 4.0 } else { -4.0 }, 0.5 + self.oleada as f32 * 0.2),
                vida: if rapido { 1 } else { 2 },
                puntos: if rapido { 50 } else { 10 * self.oleada },
            });
        }
    }

    /// Avanza un paso fijo. Devuelve los eventos ocurridos.
    pub fn actualizar(&mut self, e: Entrada) -> Vec<EventoJuego> {
        let mut ev = vec![];
        if self.terminado {
            return ev;
        }
        self.tiempo += DT;
        if self.enemigos.is_empty() {
            self.generar_oleada();
            ev.push(EventoJuego::NuevaOleada(self.oleada));
        }

        // 1) Jugador: movimiento con límites de pantalla.
        let dx = (e.derecha as i32 - e.izquierda as i32) as f32 * VEL_JUGADOR * DT;
        self.jugador.pos.x = (self.jugador.pos.x + dx).clamp(0.0, ANCHO - self.jugador.tam.x);
        self.recarga = (self.recarga - DT).max(0.0);
        self.invulnerable = (self.invulnerable - DT).max(0.0);
        if e.disparar && self.recarga == 0.0 {
            self.recarga = RECARGA;
            let p = V2::new(self.jugador.pos.x + 1.0, self.jugador.pos.y - 1.0);
            self.balas.push(Bala { caja: Aabb { pos: p, tam: V2::new(1.0, 1.0) }, vel: V2::new(0.0, -VEL_BALA), del_jugador: true });
        }

        // 2) Enemigos: rebotan en los bordes, bajan y disparan al azar.
        let prob_disparo = 0.01 + 0.005 * self.oleada as f32;
        for en in &mut self.enemigos {
            en.caja.pos = en.caja.pos + en.vel * DT;
            if en.caja.pos.x < 0.0 || en.caja.pos.x + en.caja.tam.x > ANCHO {
                en.vel.x = -en.vel.x;
                en.caja.pos.x = en.caja.pos.x.clamp(0.0, ANCHO - en.caja.tam.x);
            }
            if self.rng.f32() < prob_disparo {
                let p = V2::new(en.caja.pos.x + 0.5, en.caja.pos.y + 1.0);
                self.balas.push(Bala { caja: Aabb { pos: p, tam: V2::new(1.0, 1.0) }, vel: V2::new(0.0, VEL_BALA * 0.5), del_jugador: false });
            }
        }

        // 3) Balas: mover y descartar las que salen de pantalla.
        for b in &mut self.balas {
            b.caja.pos = b.caja.pos + b.vel * DT;
        }
        self.balas.retain(|b| b.caja.pos.y > -1.0 && b.caja.pos.y < ALTO);

        // 4) Colisiones bala del jugador ↔ enemigo.
        let mut balas_usadas = vec![false; self.balas.len()];
        for en in &mut self.enemigos {
            for (i, b) in self.balas.iter().enumerate() {
                if b.del_jugador && !balas_usadas[i] && en.vida > 0 && b.caja.choca(&en.caja) {
                    balas_usadas[i] = true;
                    en.vida -= 1;
                    if en.vida == 0 {
                        self.puntaje += en.puntos;
                        ev.push(EventoJuego::EnemigoDestruido { puntos: en.puntos });
                    }
                }
            }
        }
        let mut i = 0;
        self.balas.retain(|_| {
            i += 1;
            !balas_usadas[i - 1]
        });
        self.enemigos.retain(|e| e.vida > 0);

        // 5) Daño al jugador: balas enemigas, choque con enemigos o enemigos que llegan abajo.
        let golpe_bala = self.balas.iter().position(|b| !b.del_jugador && b.caja.choca(&self.jugador));
        let choque = self.enemigos.iter().any(|en| en.caja.choca(&self.jugador) || en.caja.pos.y >= ALTO - 1.0);
        if (golpe_bala.is_some() || choque) && self.invulnerable == 0.0 {
            if let Some(i) = golpe_bala {
                self.balas.remove(i);
            }
            if choque {
                // Los enemigos que llegaron abajo desaparecen.
                let j = self.jugador;
                self.enemigos.retain(|en| !(en.caja.choca(&j) || en.caja.pos.y >= ALTO - 1.0));
            }
            self.vidas = self.vidas.saturating_sub(1);
            self.invulnerable = 2.0;
            ev.push(EventoJuego::JugadorHerido { vidas: self.vidas });
            if self.vidas == 0 {
                self.terminado = true;
                ev.push(EventoJuego::FinDelJuego);
            }
        }
        ev
    }

    /// Dibuja el estado en un framebuffer (no requiere ventana ni GPU).
    pub fn renderizar(&self, fb: &mut Framebuffer) {
        fb.limpiar();
        for en in &self.enemigos {
            let c = if en.vida > 1 { 'W' } else { 'w' };
            for dx in 0..en.caja.tam.x as i32 {
                fb.poner(en.caja.pos.x as i32 + dx, en.caja.pos.y as i32, c, [255, 80, 80]);
            }
        }
        for b in &self.balas {
            let (c, col) = if b.del_jugador { ('|', [255, 255, 0]) } else { ('o', [255, 120, 0]) };
            fb.poner(b.caja.pos.x as i32, b.caja.pos.y as i32, c, col);
        }
        // Parpadeo durante la invulnerabilidad.
        if self.invulnerable == 0.0 || (self.tiempo * 10.0) as i32 % 2 == 0 {
            fb.texto(self.jugador.pos.x as i32, self.jugador.pos.y as i32, "/A\\", [80, 200, 255]);
        }
        fb.texto(0, 0, &format!("PTS {} VIDAS {} OLA {}", self.puntaje, self.vidas, self.oleada), [255, 255, 255]);
        if self.terminado {
            fb.texto(ANCHO as i32 / 2 - 5, ALTO as i32 / 2, "GAME OVER", [255, 0, 0]);
        }
    }
}

/// IA sencilla para demos/tests: se alinea con el enemigo más bajo y dispara.
pub fn piloto_automatico(j: &Juego) -> Entrada {
    let cx = j.jugador.pos.x + 1.5;
    let objetivo = j.enemigos.iter().max_by(|a, b| a.caja.pos.y.total_cmp(&b.caja.pos.y)).map(|e| e.caja.pos.x + 1.0);
    match objetivo {
        Some(x) => Entrada { izquierda: x < cx - 0.5, derecha: x > cx + 0.5, disparar: true },
        None => Entrada::default(),
    }
}
