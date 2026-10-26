//! Lógica pura de Snake (sin E/S): tablero, serpiente, comida, giros y puntaje.

use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dir {
    Arriba,
    Abajo,
    Izq,
    Der,
}

impl Dir {
    pub fn opuesta(self) -> Dir {
        match self {
            Dir::Arriba => Dir::Abajo,
            Dir::Abajo => Dir::Arriba,
            Dir::Izq => Dir::Der,
            Dir::Der => Dir::Izq,
        }
    }
    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::Arriba => (0, -1),
            Dir::Abajo => (0, 1),
            Dir::Izq => (-1, 0),
            Dir::Der => (1, 0),
        }
    }
}

/// Qué pasa al tocar el borde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Chocar con la pared mata.
    Paredes,
    /// El tablero es un toroide: salís por un lado y entrás por el otro.
    Toroide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resultado {
    Sigue,
    Comio,
    Murio,
    /// Llenó el tablero: ¡victoria!
    Gano,
}

pub type Pos = (i32, i32);

#[derive(Debug, Clone)]
pub struct Snake {
    pub ancho: i32,
    pub alto: i32,
    pub modo: Modo,
    /// Cuerpo: frente = cabeza.
    pub cuerpo: VecDeque<Pos>,
    pub dir: Dir,
    /// Giros pendientes (permite tocar dos teclas rápido entre ticks sin perderlas).
    cola_giros: VecDeque<Dir>,
    pub comida: Option<Pos>,
    pub puntaje: u32,
    pub vivo: bool,
    pub ticks: u64,
    rng: u64,
}

impl Snake {
    pub fn new(ancho: i32, alto: i32, modo: Modo, semilla: u64) -> Self {
        let c = (ancho / 2, alto / 2);
        let mut s = Self {
            ancho,
            alto,
            modo,
            cuerpo: VecDeque::from([c, (c.0 - 1, c.1), (c.0 - 2, c.1)]),
            dir: Dir::Der,
            cola_giros: VecDeque::new(),
            comida: None,
            puntaje: 0,
            vivo: true,
            ticks: 0,
            rng: semilla.max(1),
        };
        s.colocar_comida();
        s
    }

    fn azar(&mut self) -> u64 {
        // xorshift64: determinista para tests reproducibles.
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }

    /// Coloca la comida en una celda libre elegida uniformemente.
    pub fn colocar_comida(&mut self) {
        let libres: Vec<Pos> = (0..self.alto).flat_map(|y| (0..self.ancho).map(move |x| (x, y))).filter(|p| !self.cuerpo.contains(p)).collect();
        self.comida = if libres.is_empty() { None } else { Some(libres[(self.azar() % libres.len() as u64) as usize]) };
    }

    /// Encola un giro. Se ignoran giros de 180° y repetidos respecto del último encolado.
    pub fn girar(&mut self, d: Dir) {
        let ultima = *self.cola_giros.back().unwrap_or(&self.dir);
        if d != ultima && d != ultima.opuesta() && self.cola_giros.len() < 3 {
            self.cola_giros.push_back(d);
        }
    }

    /// Ticks entre movimientos: acelera con el puntaje (mín. 2).
    pub fn intervalo(&self) -> u32 {
        8u32.saturating_sub(self.puntaje / 5).max(2)
    }

    /// Avanza un movimiento.
    pub fn paso(&mut self) -> Resultado {
        if !self.vivo {
            return Resultado::Murio;
        }
        self.ticks += 1;
        if let Some(d) = self.cola_giros.pop_front() {
            self.dir = d;
        }
        let (dx, dy) = self.dir.delta();
        let (hx, hy) = self.cuerpo[0];
        let mut nueva = (hx + dx, hy + dy);
        let fuera = nueva.0 < 0 || nueva.1 < 0 || nueva.0 >= self.ancho || nueva.1 >= self.alto;
        if fuera {
            match self.modo {
                Modo::Paredes => {
                    self.vivo = false;
                    return Resultado::Murio;
                }
                Modo::Toroide => nueva = (nueva.0.rem_euclid(self.ancho), nueva.1.rem_euclid(self.alto)),
            }
        }
        let come = Some(nueva) == self.comida;
        // Si no come, la cola se mueve: la celda de la cola queda libre ESTE paso,
        // así que perseguir la propia cola es legal.
        if !come {
            self.cuerpo.pop_back();
        }
        if self.cuerpo.contains(&nueva) {
            self.vivo = false;
            return Resultado::Murio;
        }
        self.cuerpo.push_front(nueva);
        if come {
            self.puntaje += 1;
            self.colocar_comida();
            if self.comida.is_none() {
                return Resultado::Gano;
            }
            return Resultado::Comio;
        }
        Resultado::Sigue
    }

    /// Render a texto (bordes, cuerpo, cabeza y comida).
    pub fn renderizar(&self) -> String {
        let mut s = format!("+{}+\n", "-".repeat(self.ancho as usize));
        for y in 0..self.alto {
            s.push('|');
            for x in 0..self.ancho {
                let p = (x, y);
                s.push(if self.cuerpo.front() == Some(&p) {
                    '@'
                } else if self.cuerpo.contains(&p) {
                    'o'
                } else if self.comida == Some(p) {
                    '*'
                } else {
                    ' '
                });
            }
            s.push_str("|\n");
        }
        s + &format!("+{}+\nPuntaje: {}", "-".repeat(self.ancho as usize), self.puntaje)
    }
}
