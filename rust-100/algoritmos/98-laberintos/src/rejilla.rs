//! Rejilla 2D del laberinto.
//!
//! Formato de texto: `#` muro, `.` o espacio libre, `~` barro (coste 5),
//! `S` inicio, `E` salida. Todas las filas deben tener el mismo ancho.

use std::fmt;

/// Posición (fila, columna).
pub type Pos = (usize, usize);

/// Tipo de casilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Celda {
    /// Infranqueable.
    Muro,
    /// Coste 1.
    Libre,
    /// Coste 5: hace que el camino más corto en pasos y el más barato difieran.
    Barro,
}

impl Celda {
    /// Coste de ENTRAR en la casilla (`None` si es muro).
    pub fn coste(self) -> Option<u32> {
        match self {
            Celda::Muro => None,
            Celda::Libre => Some(1),
            Celda::Barro => Some(5),
        }
    }
}

/// Laberinto rectangular.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Laberinto {
    /// Columnas.
    pub ancho: usize,
    /// Filas.
    pub alto: usize,
    /// Casillas en orden fila‑mayor: índice = fila·ancho + columna.
    pub celdas: Vec<Celda>,
    /// Inicio.
    pub inicio: Pos,
    /// Salida.
    pub salida: Pos,
}

/// Errores de parseo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorParseo {
    /// Fila con ancho distinto (número de fila).
    AnchoIrregular(usize),
    /// Carácter desconocido.
    Caracter(char),
    /// Falta S o E, o aparece más de una vez.
    InicioOSalida,
}

impl Laberinto {
    /// Laberinto vacío (todo libre) de `alto`×`ancho`.
    pub fn abierto(alto: usize, ancho: usize) -> Self {
        Laberinto { ancho, alto, celdas: vec![Celda::Libre; ancho * alto], inicio: (0, 0), salida: (alto - 1, ancho - 1) }
    }

    /// Parsea el formato de texto.
    pub fn parsear(texto: &str) -> Result<Self, ErrorParseo> {
        let filas: Vec<&str> = texto.lines().filter(|l| !l.is_empty()).collect();
        let ancho = filas.first().map_or(0, |f| f.chars().count());
        let mut celdas = Vec::with_capacity(ancho * filas.len());
        let (mut inicio, mut salida) = (None, None);
        for (r, fila) in filas.iter().enumerate() {
            if fila.chars().count() != ancho {
                return Err(ErrorParseo::AnchoIrregular(r));
            }
            for (c, ch) in fila.chars().enumerate() {
                celdas.push(match ch {
                    '#' => Celda::Muro,
                    '.' | ' ' => Celda::Libre,
                    '~' => Celda::Barro,
                    'S' | 'E' => {
                        let destino = if ch == 'S' { &mut inicio } else { &mut salida };
                        if destino.replace((r, c)).is_some() {
                            return Err(ErrorParseo::InicioOSalida);
                        }
                        Celda::Libre
                    }
                    otro => return Err(ErrorParseo::Caracter(otro)),
                });
            }
        }
        match (inicio, salida) {
            (Some(inicio), Some(salida)) => Ok(Laberinto { ancho, alto: filas.len(), celdas, inicio, salida }),
            _ => Err(ErrorParseo::InicioOSalida),
        }
    }

    /// Casilla en `p`.
    pub fn celda(&self, p: Pos) -> Celda {
        self.celdas[p.0 * self.ancho + p.1]
    }

    /// Cambia la casilla en `p`.
    pub fn poner(&mut self, p: Pos, c: Celda) {
        self.celdas[p.0 * self.ancho + p.1] = c;
    }

    /// Índice lineal (para arrays de distancias/visitados, más rápidos que un HashMap).
    pub fn indice(&self, p: Pos) -> usize {
        p.0 * self.ancho + p.1
    }

    /// Vecinos transitables (arriba, derecha, abajo, izquierda). El orden
    /// fijo hace que los resultados sean deterministas. `checked_sub`
    /// evita el *underflow* en los bordes.
    pub fn vecinos(&self, (f, c): Pos) -> impl Iterator<Item = Pos> + '_ {
        [
            f.checked_sub(1).map(|f| (f, c)),
            (c + 1 < self.ancho).then_some((f, c + 1)),
            (f + 1 < self.alto).then_some((f + 1, c)),
            c.checked_sub(1).map(|c| (f, c)),
        ]
        .into_iter()
        .flatten()
        .filter(|&p| self.celda(p) != Celda::Muro)
    }

    /// Dibuja el laberinto marcando un camino con `*` (`o` si pasa por barro).
    pub fn dibujar(&self, camino: &[Pos]) -> String {
        let mut marcas = vec![false; self.celdas.len()];
        for &p in camino {
            marcas[self.indice(p)] = true;
        }
        let mut s = String::with_capacity((self.ancho + 1) * self.alto);
        for f in 0..self.alto {
            for c in 0..self.ancho {
                let p = (f, c);
                s.push(if p == self.inicio {
                    'S'
                } else if p == self.salida {
                    'E'
                } else {
                    match (self.celda(p), marcas[self.indice(p)]) {
                        (Celda::Muro, _) => '#',
                        (Celda::Libre, true) => '*',
                        (Celda::Barro, true) => 'o',
                        (Celda::Libre, false) => ' ',
                        (Celda::Barro, false) => '~',
                    }
                });
            }
            s.push('\n');
        }
        s
    }
}

impl fmt::Display for Laberinto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.dibujar(&[]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parseo_y_dibujo_ida_y_vuelta() {
        let txt = "#####\n#S~E#\n#####\n";
        let l = Laberinto::parsear(txt).unwrap();
        assert_eq!((l.alto, l.ancho, l.inicio, l.salida), (3, 5, (1, 1), (1, 3)));
        assert_eq!(l.celda((1, 2)), Celda::Barro);
        assert_eq!(l.to_string(), txt);
    }

    #[test]
    fn errores() {
        assert_eq!(Laberinto::parsear("S.\n.").unwrap_err(), ErrorParseo::AnchoIrregular(1));
        assert_eq!(Laberinto::parsear("S?E").unwrap_err(), ErrorParseo::Caracter('?'));
        assert_eq!(Laberinto::parsear("S..").unwrap_err(), ErrorParseo::InicioOSalida);
        assert_eq!(Laberinto::parsear("SSE").unwrap_err(), ErrorParseo::InicioOSalida);
    }

    #[test]
    fn vecinos_en_esquina() {
        let l = Laberinto::abierto(3, 3);
        let v: Vec<_> = l.vecinos((0, 0)).collect();
        assert_eq!(v, vec![(0, 1), (1, 0)]);
    }
}
