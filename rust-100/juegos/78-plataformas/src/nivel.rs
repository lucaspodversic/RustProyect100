//! Niveles definidos como mapas ASCII (fácil de editar y de testear).
//!
//! Leyenda: `#` sólido · `=` plataforma de un sentido (se atraviesa desde
//! abajo) · `^` pinchos · `o` moneda · `P` inicio del jugador · `F` meta.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    Vacio,
    Solido,
    UnSentido,
    Pinchos,
    Moneda,
    Meta,
}

#[derive(Debug, Clone)]
pub struct Nivel {
    pub ancho: usize,
    pub alto: usize,
    pub tiles: Vec<Tile>,
    pub inicio: (f32, f32),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ErrorNivel {
    Vacio,
    SinInicio,
    CaracterInvalido(char),
}

impl Nivel {
    pub fn parsear(txt: &str) -> Result<Nivel, ErrorNivel> {
        let filas: Vec<&str> = txt.lines().filter(|l| !l.is_empty()).collect();
        if filas.is_empty() {
            return Err(ErrorNivel::Vacio);
        }
        let ancho = filas.iter().map(|f| f.chars().count()).max().unwrap();
        let alto = filas.len();
        let mut tiles = vec![Tile::Vacio; ancho * alto];
        let mut inicio = None;
        for (y, f) in filas.iter().enumerate() {
            for (x, c) in f.chars().enumerate() {
                tiles[y * ancho + x] = match c {
                    '#' => Tile::Solido,
                    '=' => Tile::UnSentido,
                    '^' => Tile::Pinchos,
                    'o' => Tile::Moneda,
                    'F' => Tile::Meta,
                    'P' => {
                        inicio = Some((x as f32, y as f32));
                        Tile::Vacio
                    }
                    ' ' | '.' => Tile::Vacio,
                    otro => return Err(ErrorNivel::CaracterInvalido(otro)),
                };
            }
        }
        Ok(Nivel { ancho, alto, tiles, inicio: inicio.ok_or(ErrorNivel::SinInicio)? })
    }

    /// Tile en (x, y). Fuera de los lados = sólido (paredes invisibles);
    /// arriba = vacío; abajo = vacío (caerse del mapa mata).
    pub fn tile(&self, x: i32, y: i32) -> Tile {
        if x < 0 || x >= self.ancho as i32 {
            return Tile::Solido;
        }
        if y < 0 || y >= self.alto as i32 {
            return Tile::Vacio;
        }
        self.tiles[y as usize * self.ancho + x as usize]
    }

    pub fn poner(&mut self, x: i32, y: i32, t: Tile) {
        if x >= 0 && y >= 0 && (x as usize) < self.ancho && (y as usize) < self.alto {
            self.tiles[y as usize * self.ancho + x as usize] = t;
        }
    }

    pub fn monedas(&self) -> usize {
        self.tiles.iter().filter(|t| **t == Tile::Moneda).count()
    }
}

pub const NIVEL_1: &str = "
#                                        #
#                                  o  F  #
#                    o o        ======== #
#                  =======               #
#          o                             #
#        =====          ^^^              #
#  P            ####   #####    o        #
######   ##########################  #####
######^^^##########################^^#####
";
