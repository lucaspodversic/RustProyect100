//! Estado del cubo y aplicación de movimientos en notación Singmaster.

use crate::geometria::{permutacion, punto3, NORMALES};
use std::fmt;
use std::sync::OnceLock;

/// Colores: U blanco, R rojo, F verde, D amarillo, L naranja, B azul.
pub const COLORES: [char; 6] = ['W', 'R', 'G', 'Y', 'O', 'B'];

/// Ejes base de giro: caras U R F D L B, capas medias M E S, rotaciones x y z.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Eje {
    U, R, F, D, L, B, M, E, S, X, Y, Z,
}

impl Eje {
    pub const TODOS: [Eje; 12] = [Eje::U, Eje::R, Eje::F, Eje::D, Eje::L, Eje::B, Eje::M, Eje::E, Eje::S, Eje::X, Eje::Y, Eje::Z];
    pub const CARAS: [Eje; 6] = [Eje::U, Eje::R, Eje::F, Eje::D, Eje::L, Eje::B];

    fn letra(self) -> char {
        ['U', 'R', 'F', 'D', 'L', 'B', 'M', 'E', 'S', 'x', 'y', 'z'][self as usize]
    }

    /// Eje de cara opuesta (para podar búsquedas: "R L" = "L R").
    pub fn opuesto(self) -> Option<Eje> {
        Some(match self {
            Eje::U => Eje::D, Eje::D => Eje::U, Eje::R => Eje::L, Eje::L => Eje::R, Eje::F => Eje::B, Eje::B => Eje::F,
            _ => return None,
        })
    }
}

/// Movimiento: eje + cantidad de cuartos de vuelta horarios (1, 2 o 3 = ').
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mov {
    pub eje: Eje,
    pub giros: u8,
}

impl Mov {
    pub fn inversa(self) -> Mov {
        Mov { eje: self.eje, giros: (4 - self.giros) % 4 }
    }
}

impl fmt::Display for Mov {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.eje.letra(), ["", "", "2", "'"][self.giros as usize])
    }
}

/// Tablas de permutación (cuarto de vuelta) calculadas una sola vez.
fn tablas() -> &'static [[u8; 54]; 12] {
    static T: OnceLock<[[u8; 54]; 12]> = OnceLock::new();
    T.get_or_init(|| {
        let n = NORMALES;
        let neg = |v: [i32; 3]| [-v[0], -v[1], -v[2]];
        Eje::TODOS.map(|e| match e {
            // Cara: rota la capa cuya proyección sobre la normal es 1.
            Eje::U | Eje::R | Eje::F | Eje::D | Eje::L | Eje::B => {
                let a = n[e as usize];
                permutacion(a, move |p| punto3(p, a) == 1)
            }
            // Capas medias: M sigue a L, E sigue a D, S sigue a F.
            Eje::M => permutacion(neg(n[1]), |p| p[0] == 0),
            Eje::E => permutacion(n[3], |p| p[1] == 0),
            Eje::S => permutacion(n[2], |p| p[2] == 0),
            // Rotaciones de cubo completo: x sigue a R, y a U, z a F.
            Eje::X => permutacion(n[1], |_| true),
            Eje::Y => permutacion(n[0], |_| true),
            Eje::Z => permutacion(n[2], |_| true),
        })
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cubo {
    /// 54 stickers (índice de color 0..6), cara*9 + fila*3 + col.
    pub s: [u8; 54],
}

impl Default for Cubo {
    fn default() -> Self {
        Self::resuelto()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ErrorNotacion(pub String);

impl Cubo {
    pub fn resuelto() -> Self {
        Self { s: std::array::from_fn(|i| (i / 9) as u8) }
    }

    /// Resuelto = cada cara de un solo color (admite el cubo rotado entero).
    pub fn esta_resuelto(&self) -> bool {
        self.s.chunks(9).all(|c| c.iter().all(|&x| x == c[4]))
    }

    pub fn aplicar(&mut self, m: Mov) {
        let p = &tablas()[m.eje as usize];
        for _ in 0..m.giros {
            let mut nuevo = [0u8; 54];
            for i in 0..54 {
                nuevo[p[i] as usize] = self.s[i];
            }
            self.s = nuevo;
        }
    }

    pub fn aplicar_seq(&mut self, seq: &[Mov]) {
        seq.iter().for_each(|m| self.aplicar(*m));
    }

    pub fn ejecutar(&mut self, notacion: &str) -> Result<(), ErrorNotacion> {
        let seq = parsear(notacion)?;
        self.aplicar_seq(&seq);
        Ok(())
    }

    /// Red desplegada:     U
    ///                   L F R B
    ///                     D
    pub fn red(&self, ansi: bool) -> String {
        let c = |cara: usize, f: usize, k: usize| {
            let v = self.s[cara * 9 + f * 3 + k] as usize;
            if ansi {
                let codigo = [97, 91, 92, 93, 33, 94][v];
                format!("\x1b[{codigo}m{}\x1b[0m", COLORES[v])
            } else {
                COLORES[v].to_string()
            }
        };
        let fila = |cara: usize, f: usize| (0..3).map(|k| c(cara, f, k)).collect::<String>();
        let mut out = String::new();
        for f in 0..3 {
            out += &format!("    {}\n", fila(0, f));
        }
        for f in 0..3 {
            out += &format!("{} {} {} {}\n", fila(4, f), fila(2, f), fila(1, f), fila(5, f));
        }
        for f in 0..3 {
            out += &format!("    {}\n", fila(3, f));
        }
        out
    }
}

/// Parsea "R U R' U2 M x'" (también acepta minúsculas para x y z y "2'").
pub fn parsear(s: &str) -> Result<Vec<Mov>, ErrorNotacion> {
    s.split_whitespace()
        .map(|tok| {
            let mut ch = tok.chars();
            let eje = match ch.next() {
                Some('U') => Eje::U, Some('R') => Eje::R, Some('F') => Eje::F,
                Some('D') => Eje::D, Some('L') => Eje::L, Some('B') => Eje::B,
                Some('M') => Eje::M, Some('E') => Eje::E, Some('S') => Eje::S,
                Some('x') => Eje::X, Some('y') => Eje::Y, Some('z') => Eje::Z,
                _ => return Err(ErrorNotacion(tok.into())),
            };
            let giros = match ch.as_str() {
                "" => 1,
                "2" | "2'" => 2,
                "'" => 3,
                _ => return Err(ErrorNotacion(tok.into())),
            };
            Ok(Mov { eje, giros })
        })
        .collect()
}

pub fn a_texto(seq: &[Mov]) -> String {
    seq.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(" ")
}

/// Inversa de una secuencia: invertir el orden y cada movimiento.
pub fn inversa(seq: &[Mov]) -> Vec<Mov> {
    seq.iter().rev().map(|m| m.inversa()).collect()
}

/// Orden de una secuencia: cuántas repeticiones vuelven al estado inicial.
pub fn orden(seq: &[Mov]) -> u32 {
    let mut c = Cubo::resuelto();
    for n in 1..=10_000 {
        c.aplicar_seq(seq);
        if c == Cubo::resuelto() {
            return n;
        }
    }
    0
}
