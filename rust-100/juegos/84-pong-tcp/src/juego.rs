//! Lógica pura de Pong (sin red ni E/S): determinista y testeable.

pub const ANCHO: f32 = 80.0;
pub const ALTO: f32 = 24.0;
pub const ALTO_PALETA: f32 = 5.0;
pub const X_PALETA: [f32; 2] = [2.0, ANCHO - 2.0];
pub const VEL_PALETA: f32 = 22.0;
pub const VEL_INICIAL: f32 = 28.0;
pub const VEL_MAX: f32 = 70.0;
/// Ángulo máximo de salida al pegar en el extremo de la paleta (≈ 50°).
pub const ANGULO_MAX: f32 = 0.87;

#[derive(Debug, Clone, PartialEq)]
pub struct Estado {
    /// Centro vertical de cada paleta.
    pub paletas: [f32; 2],
    pub pelota: (f32, f32),
    pub vel: (f32, f32),
    pub puntos: [u32; 2],
    pub tick: u64,
    pub ganador: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Evento {
    Rebote(usize),
    Pared,
    Punto(usize),
    Fin(usize),
}

pub struct Juego {
    pub estado: Estado,
    pub para_ganar: u32,
    /// Saques alternados: el que recibió el punto saca hacia el rival.
    saques: u32,
}

impl Juego {
    pub fn new(para_ganar: u32) -> Self {
        let mut j = Self {
            estado: Estado { paletas: [ALTO / 2.0; 2], pelota: (0.0, 0.0), vel: (0.0, 0.0), puntos: [0, 0], tick: 0, ganador: None },
            para_ganar,
            saques: 0,
        };
        j.sacar(1);
        j
    }

    /// Pone la pelota en el centro moviéndose hacia `hacia` (0 = izquierda),
    /// con un ángulo que varía de forma determinista entre saques.
    fn sacar(&mut self, hacia: usize) {
        self.saques += 1;
        let ang = [0.35f32, -0.25, 0.15, -0.45, 0.3][self.saques as usize % 5];
        let dir = if hacia == 0 { -1.0 } else { 1.0 };
        self.estado.pelota = (ANCHO / 2.0, ALTO / 2.0);
        self.estado.vel = (dir * VEL_INICIAL * ang.cos(), VEL_INICIAL * ang.sin());
    }

    /// Avanza `dt` segundos con las entradas de cada jugador (−1 arriba, 0, +1 abajo).
    pub fn paso(&mut self, entradas: [i8; 2], dt: f32) -> Vec<Evento> {
        let mut ev = vec![];
        let e = &mut self.estado;
        if e.ganador.is_some() {
            return ev;
        }
        e.tick += 1;
        for (p, &d) in e.paletas.iter_mut().zip(&entradas) {
            *p = (*p + d.signum() as f32 * VEL_PALETA * dt).clamp(ALTO_PALETA / 2.0, ALTO - ALTO_PALETA / 2.0);
        }
        let (x0, _) = e.pelota;
        e.pelota.0 += e.vel.0 * dt;
        e.pelota.1 += e.vel.1 * dt;
        // Paredes superior/inferior: reflejo.
        if e.pelota.1 < 0.0 {
            e.pelota.1 = -e.pelota.1;
            e.vel.1 = e.vel.1.abs();
            ev.push(Evento::Pared);
        } else if e.pelota.1 > ALTO {
            e.pelota.1 = 2.0 * ALTO - e.pelota.1;
            e.vel.1 = -e.vel.1.abs();
            ev.push(Evento::Pared);
        }
        // Paletas: detectamos el CRUCE de la línea de la paleta en este paso
        // (evita que la pelota atraviese la paleta a alta velocidad).
        for j in 0..2 {
            let xp = X_PALETA[j];
            let hacia_ella = if j == 0 { e.vel.0 < 0.0 } else { e.vel.0 > 0.0 };
            let cruzo = if j == 0 { x0 >= xp && e.pelota.0 <= xp } else { x0 <= xp && e.pelota.0 >= xp };
            if hacia_ella && cruzo {
                let offset = (e.pelota.1 - e.paletas[j]) / (ALTO_PALETA / 2.0);
                if offset.abs() <= 1.1 {
                    // El ángulo de salida depende de dónde pegó (control del jugador).
                    let rapidez = ((e.vel.0.powi(2) + e.vel.1.powi(2)).sqrt() * 1.06).min(VEL_MAX);
                    let ang = offset.clamp(-1.0, 1.0) * ANGULO_MAX;
                    let dir = if j == 0 { 1.0 } else { -1.0 };
                    e.vel = (dir * rapidez * ang.cos(), rapidez * ang.sin());
                    e.pelota.0 = xp + dir * 0.01;
                    ev.push(Evento::Rebote(j));
                }
            }
        }
        // Punto: la pelota salió por un costado.
        let anotador = if e.pelota.0 < 0.0 { Some(1) } else if e.pelota.0 > ANCHO { Some(0) } else { None };
        if let Some(a) = anotador {
            e.puntos[a] += 1;
            ev.push(Evento::Punto(a));
            if e.puntos[a] >= self.para_ganar {
                e.ganador = Some(a);
                ev.push(Evento::Fin(a));
            } else {
                self.sacar(1 - a); // saca hacia el que perdió el punto
            }
        }
        ev
    }
}

/// IA simple: sigue la pelota cuando viene hacia su lado, si no vuelve al centro.
/// `zona_muerta` evita el temblor; una IA "imperfecta" usa una zona mayor.
pub fn ia(e: &Estado, jugador: usize, zona_muerta: f32) -> i8 {
    let viene = if jugador == 0 { e.vel.0 < 0.0 } else { e.vel.0 > 0.0 };
    let objetivo = if viene { e.pelota.1 } else { ALTO / 2.0 };
    let d = objetivo - e.paletas[jugador];
    if d > zona_muerta {
        1
    } else if d < -zona_muerta {
        -1
    } else {
        0
    }
}

/// Render ASCII del estado (80×24 → `cols`×`filas`).
pub fn render(e: &Estado, cols: usize, filas: usize) -> String {
    let mut g = vec![vec![' '; cols]; filas];
    let sx = |x: f32| ((x / ANCHO * cols as f32) as usize).min(cols - 1);
    let sy = |y: f32| ((y / ALTO * filas as f32) as usize).min(filas - 1);
    for (fila, linea) in g.iter_mut().enumerate() {
        if fila % 2 == 0 {
            linea[cols / 2] = '┊';
        }
    }
    for j in 0..2 {
        let (a, b) = (sy(e.paletas[j] - ALTO_PALETA / 2.0), sy(e.paletas[j] + ALTO_PALETA / 2.0 - 0.01));
        for fila in g.iter_mut().take(b + 1).skip(a) {
            fila[sx(X_PALETA[j])] = '█';
        }
    }
    g[sy(e.pelota.1.clamp(0.0, ALTO))][sx(e.pelota.0.clamp(0.0, ANCHO))] = '●';
    let mut s = format!("{:^w$}\n", format!("{}  :  {}", e.puntos[0], e.puntos[1]), w = cols);
    s += &format!("+{}+\n", "-".repeat(cols));
    for f in g {
        s += &format!("|{}|\n", f.into_iter().collect::<String>());
    }
    s += &format!("+{}+\n", "-".repeat(cols));
    s
}
