//! Sistema de partículas: integración, fuerzas, emisores, colisiones.

use crate::grilla::Grilla;
use crate::vec2::{v2, V2};

#[derive(Debug, Clone, PartialEq)]
pub struct Particula {
    pub pos: V2,
    pub vel: V2,
    pub masa: f32,
    pub radio: f32,
    /// Segundos de vida restantes (`f32::INFINITY` = inmortal).
    pub vida: f32,
}

/// Fuerzas globales que actúan sobre todas las partículas.
#[derive(Debug, Clone)]
pub enum Fuerza {
    /// Aceleración constante (no depende de la masa).
    Gravedad(V2),
    /// Fuerza constante (sí depende de la masa: a = F/m).
    Viento(V2),
    /// Atractor puntual con intensidad `k` (negativa = repulsor), suavizado
    /// con `eps` para evitar la singularidad en distancia 0.
    Atractor { centro: V2, k: f32, eps: f32 },
    /// Arrastre lineal: F = −c·v.
    Arrastre(f32),
}

/// Emisor: genera `tasa` partículas/segundo con velocidad en un cono.
#[derive(Debug, Clone)]
pub struct Emisor {
    pub pos: V2,
    pub tasa: f32,
    pub direccion: f32, // radianes
    pub apertura: f32,  // ± radianes
    pub rapidez: (f32, f32),
    pub vida: (f32, f32),
    acumulado: f32,
}

impl Emisor {
    pub fn new(pos: V2, tasa: f32, direccion: f32, apertura: f32) -> Self {
        Self { pos, tasa, direccion, apertura, rapidez: (4.0, 8.0), vida: (1.0, 3.0), acumulado: 0.0 }
    }
}

pub struct Sistema {
    pub ancho: f32,
    pub alto: f32,
    pub particulas: Vec<Particula>,
    pub fuerzas: Vec<Fuerza>,
    pub emisores: Vec<Emisor>,
    /// Coeficiente de restitución contra las paredes (1 = elástico).
    pub restitucion: f32,
    pub colisiones: bool,
    pub max_particulas: usize,
    rng: u64,
    grilla: Grilla,
}

impl Sistema {
    pub fn new(ancho: f32, alto: f32, semilla: u64) -> Self {
        Self {
            ancho,
            alto,
            particulas: vec![],
            fuerzas: vec![],
            emisores: vec![],
            restitucion: 0.8,
            colisiones: false,
            max_particulas: 5000,
            rng: semilla.max(1),
            grilla: Grilla::new(1.0),
        }
    }

    /// Número pseudoaleatorio uniforme en [a, b).
    fn azar(&mut self, a: f32, b: f32) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        a + (b - a) * ((self.rng >> 40) as f32 / (1u64 << 24) as f32)
    }

    pub fn agregar(&mut self, pos: V2, vel: V2) {
        self.particulas.push(Particula { pos, vel, masa: 1.0, radio: 0.25, vida: f32::INFINITY });
    }

    fn aceleracion(&self, p: &Particula) -> V2 {
        let mut a = V2::default();
        for f in &self.fuerzas {
            a += match *f {
                Fuerza::Gravedad(g) => g,
                Fuerza::Viento(w) => w * (1.0 / p.masa),
                Fuerza::Atractor { centro, k, eps } => {
                    let d = centro - p.pos;
                    let r2 = d.len2() + eps * eps;
                    d * (k / (r2 * r2.sqrt()) / p.masa)
                }
                Fuerza::Arrastre(c) => p.vel * (-c / p.masa),
            };
        }
        a
    }

    /// Avanza `dt` segundos.
    pub fn paso(&mut self, dt: f32) {
        // 1) Emisores (acumulador fraccional: tasa 2.5/s funciona bien).
        for e in 0..self.emisores.len() {
            self.emisores[e].acumulado += self.emisores[e].tasa * dt;
            while self.emisores[e].acumulado >= 1.0 && self.particulas.len() < self.max_particulas {
                self.emisores[e].acumulado -= 1.0;
                let em = self.emisores[e].clone();
                let ang = em.direccion + self.azar(-em.apertura, em.apertura);
                let rap = self.azar(em.rapidez.0, em.rapidez.1);
                let vida = self.azar(em.vida.0, em.vida.1);
                self.particulas.push(Particula { pos: em.pos, vel: v2(ang.cos(), ang.sin()) * rap, masa: 1.0, radio: 0.2, vida });
            }
            self.emisores[e].acumulado = self.emisores[e].acumulado.min(1.0);
        }
        // 2) Integración Euler semi-implícita (simpléctica): primero v, luego x.
        //    Es estable para órbitas y conserva energía mucho mejor que Euler explícito.
        for i in 0..self.particulas.len() {
            let a = self.aceleracion(&self.particulas[i]);
            let p = &mut self.particulas[i];
            p.vel += a * dt;
            p.pos += p.vel * dt;
            p.vida -= dt;
        }
        // 3) Vida: eliminar las vencidas (swap_remove es O(1)).
        let mut i = 0;
        while i < self.particulas.len() {
            if self.particulas[i].vida <= 0.0 {
                self.particulas.swap_remove(i);
            } else {
                i += 1;
            }
        }
        // 4) Paredes.
        let (w, h, e) = (self.ancho, self.alto, self.restitucion);
        for p in &mut self.particulas {
            rebotar(&mut p.pos.x, &mut p.vel.x, p.radio, w, e);
            rebotar(&mut p.pos.y, &mut p.vel.y, p.radio, h, e);
        }
        // 5) Colisiones entre partículas.
        if self.colisiones {
            self.resolver_colisiones();
        }
    }

    fn resolver_colisiones(&mut self) {
        let max_r = self.particulas.iter().map(|p| p.radio).fold(0.0, f32::max);
        if max_r == 0.0 {
            return;
        }
        if (self.grilla.celda - 2.0 * max_r).abs() > 1e-6 {
            self.grilla = Grilla::new(2.0 * max_r);
        }
        let pos: Vec<V2> = self.particulas.iter().map(|p| p.pos).collect();
        self.grilla.reconstruir(pos.iter().copied());
        // Orden determinista (el HashMap itera en orden arbitrario).
        let mut pares = self.grilla.pares(&pos, 2.0 * max_r);
        pares.sort_unstable();
        for (i, j) in pares {
            colision_elastica(&mut self.particulas, i, j);
        }
    }

    pub fn energia_cinetica(&self) -> f32 {
        self.particulas.iter().map(|p| 0.5 * p.masa * p.vel.len2()).sum()
    }

    pub fn momento(&self) -> V2 {
        self.particulas.iter().fold(V2::default(), |acc, p| acc + p.vel * p.masa)
    }

    /// Render ASCII por densidad (más partículas en la celda = carácter más denso).
    pub fn render(&self, cols: usize, filas: usize) -> String {
        let mut cuenta = vec![0u32; cols * filas];
        for p in &self.particulas {
            let cx = (p.pos.x / self.ancho * cols as f32) as isize;
            // y crece hacia arriba en el mundo, hacia abajo en pantalla.
            let cy = filas as isize - 1 - (p.pos.y / self.alto * filas as f32) as isize;
            if cx >= 0 && cy >= 0 && (cx as usize) < cols && (cy as usize) < filas {
                cuenta[cy as usize * cols + cx as usize] += 1;
            }
        }
        const RAMPA: [char; 6] = [' ', '.', ':', '*', '#', '@'];
        let mut s = String::with_capacity((cols + 1) * filas);
        for f in 0..filas {
            for c in 0..cols {
                s.push(RAMPA[(cuenta[f * cols + c] as usize).min(5)]);
            }
            s.push('\n');
        }
        s
    }
}

/// Rebote contra [radio, limite − radio] en un eje.
fn rebotar(x: &mut f32, v: &mut f32, r: f32, limite: f32, e: f32) {
    if *x < r {
        *x = r + (r - *x) * e;
        *v = v.abs() * e;
    } else if *x > limite - r {
        *x = limite - r - (*x - (limite - r)) * e;
        *v = -v.abs() * e;
    }
    *x = x.clamp(r, (limite - r).max(r));
}

/// Choque elástico entre esferas: intercambia la componente normal de la
/// velocidad (ponderada por masas) y separa para quitar el solapamiento.
pub fn colision_elastica(ps: &mut [Particula], i: usize, j: usize) {
    let (a, b) = (&ps[i], &ps[j]);
    let d = b.pos - a.pos;
    let dist = d.len();
    let min = a.radio + b.radio;
    if dist >= min || dist == 0.0 {
        return;
    }
    let n = d * (1.0 / dist);
    let v_rel = (b.vel - a.vel).dot(n);
    let (ma, mb) = (a.masa, b.masa);
    // Separación proporcional a la masa inversa.
    let solape = min - dist;
    let corr = n * (solape / (ma + mb));
    ps[i].pos -= corr * mb;
    ps[j].pos += corr * ma;
    if v_rel >= 0.0 {
        return; // ya se están alejando
    }
    let imp = 2.0 * v_rel / (ma + mb);
    ps[i].vel += n * (imp * mb);
    ps[j].vel -= n * (imp * ma);
}
