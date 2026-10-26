//! Máquina de Turing determinista de una cinta.
//!
//! ## Formato de texto
//!
//! ```text
//! # comentario
//! inicio: q0
//! aceptar: si          (opcional; puede haber varios separados por espacios)
//! rechazar: no         (opcional)
//! blanco: _            (opcional, por defecto '_')
//! q0 1 -> q0 1 R       estado símbolo -> nuevo_estado escribe movimiento(L/R/S)
//! ```
//!
//! Si no existe transición para (estado, símbolo), la máquina se **detiene**.
//! Si se detiene en un estado de aceptación, el resultado es `Acepta`; si lo
//! hace en uno de rechazo (o en otro estado sin transición), es `Rechaza` o
//! `Detenida`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

/// Movimiento del cabezal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mov {
    /// Izquierda.
    L,
    /// Derecha.
    R,
    /// Quedarse (*stay*).
    S,
}

/// Lo que hace una transición.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accion {
    /// Estado siguiente.
    pub estado: String,
    /// Símbolo que se escribe.
    pub escribe: char,
    /// Movimiento.
    pub mov: Mov,
}

/// Cinta "infinita" en ambos sentidos: una `VecDeque` que crece por el
/// extremo que haga falta. `origen` recuerda dónde está la celda 0 lógica
/// para poder imprimir posiciones estables.
#[derive(Debug, Clone)]
pub struct Cinta {
    celdas: VecDeque<char>,
    cabezal: usize,
    blanco: char,
}

impl Cinta {
    /// Crea la cinta con la entrada y el cabezal sobre el primer símbolo.
    pub fn nueva(entrada: &str, blanco: char) -> Self {
        let mut celdas: VecDeque<char> = entrada.chars().collect();
        if celdas.is_empty() {
            celdas.push_back(blanco);
        }
        Cinta { celdas, cabezal: 0, blanco }
    }

    /// Símbolo bajo el cabezal.
    pub fn leer(&self) -> char {
        self.celdas[self.cabezal]
    }

    /// Escribe y mueve. Al salirse por la izquierda se inserta un blanco
    /// delante (`push_front`, O(1) amortizado) y el índice sigue siendo 0.
    pub fn escribir_y_mover(&mut self, c: char, m: Mov) {
        self.celdas[self.cabezal] = c;
        match m {
            Mov::L => {
                if self.cabezal == 0 {
                    self.celdas.push_front(self.blanco);
                } else {
                    self.cabezal -= 1;
                }
            }
            Mov::R => {
                self.cabezal += 1;
                if self.cabezal == self.celdas.len() {
                    self.celdas.push_back(self.blanco);
                }
            }
            Mov::S => {}
        }
    }

    /// Contenido sin los blancos de los extremos.
    pub fn contenido(&self) -> String {
        let s: String = self.celdas.iter().collect();
        s.trim_matches(self.blanco).to_string()
    }

    /// Número de símbolos distintos del blanco (útil para el castor afanoso).
    pub fn no_blancos(&self) -> usize {
        self.celdas.iter().filter(|&&c| c != self.blanco).count()
    }
}

impl fmt::Display for Cinta {
    /// Muestra la cinta con el cabezal entre corchetes: `1 0 [1] _`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, c) in self.celdas.iter().enumerate() {
            if i == self.cabezal {
                write!(f, "[{c}]")?;
            } else {
                write!(f, " {c} ")?;
            }
        }
        Ok(())
    }
}

/// Resultado de una ejecución.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resultado {
    /// Se detuvo en un estado de aceptación.
    Acepta,
    /// Se detuvo en un estado de rechazo.
    Rechaza,
    /// Se detuvo (sin transición) en un estado que no es ni de aceptación ni de rechazo.
    Detenida(String),
    /// Se alcanzó el límite de pasos (quizá no pare nunca).
    LimiteExcedido,
}

/// Informe de ejecución.
#[derive(Debug, Clone)]
pub struct Ejecucion {
    /// Resultado.
    pub resultado: Resultado,
    /// Pasos (transiciones) ejecutados.
    pub pasos: u64,
    /// Cinta final.
    pub cinta: Cinta,
}

/// Errores del formato de texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorFormato {
    /// Línea mal formada (número de línea, 1‑based).
    Linea(usize, String),
    /// Falta `inicio:`.
    SinInicio,
    /// Dos transiciones para el mismo (estado, símbolo): no sería determinista.
    NoDeterminista(usize),
}

impl fmt::Display for ErrorFormato {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorFormato::Linea(n, m) => write!(f, "línea {n}: {m}"),
            ErrorFormato::SinInicio => write!(f, "falta la directiva `inicio:`"),
            ErrorFormato::NoDeterminista(n) => write!(f, "línea {n}: transición duplicada (no determinista)"),
        }
    }
}

impl std::error::Error for ErrorFormato {}

/// Máquina de Turing.
#[derive(Debug, Clone)]
pub struct Maquina {
    /// Estado inicial.
    pub inicio: String,
    /// Estados de aceptación.
    pub aceptar: HashSet<String>,
    /// Estados de rechazo.
    pub rechazar: HashSet<String>,
    /// Símbolo blanco.
    pub blanco: char,
    /// Función de transición δ(estado, símbolo).
    pub delta: HashMap<(String, char), Accion>,
}

impl Maquina {
    /// Parsea el formato de texto descrito en el módulo.
    pub fn parsear(texto: &str) -> Result<Self, ErrorFormato> {
        let mut inicio = None;
        let mut aceptar = HashSet::new();
        let mut rechazar = HashSet::new();
        let mut blanco = '_';
        let mut delta = HashMap::new();
        for (i, linea) in texto.lines().enumerate() {
            let n = i + 1;
            // Quitamos comentarios y espacios.
            let linea = linea.split('#').next().unwrap_or("").trim();
            if linea.is_empty() {
                continue;
            }
            if let Some((clave, valor)) = linea.split_once(':') {
                let valor = valor.trim();
                match clave.trim() {
                    "inicio" => inicio = Some(valor.to_string()),
                    "aceptar" => aceptar.extend(valor.split_whitespace().map(String::from)),
                    "rechazar" => rechazar.extend(valor.split_whitespace().map(String::from)),
                    "blanco" => {
                        let mut cs = valor.chars();
                        blanco = match (cs.next(), cs.next()) {
                            (Some(c), None) => c,
                            _ => return Err(ErrorFormato::Linea(n, "el blanco debe ser un carácter".into())),
                        }
                    }
                    otra => return Err(ErrorFormato::Linea(n, format!("directiva desconocida `{otra}`"))),
                }
                continue;
            }
            // Transición: 6 tokens con "->" en medio.
            let t: Vec<&str> = linea.split_whitespace().collect();
            let simbolo = |s: &str| {
                let mut cs = s.chars();
                match (cs.next(), cs.next()) {
                    (Some(c), None) => Ok(c),
                    _ => Err(ErrorFormato::Linea(n, format!("`{s}` no es un único símbolo"))),
                }
            };
            if t.len() != 6 || t[2] != "->" {
                return Err(ErrorFormato::Linea(n, "se esperaba `estado símbolo -> estado escribe L|R|S`".into()));
            }
            let mov = match t[5] {
                "L" => Mov::L,
                "R" => Mov::R,
                "S" => Mov::S,
                m => return Err(ErrorFormato::Linea(n, format!("movimiento `{m}` inválido"))),
            };
            let clave = (t[0].to_string(), simbolo(t[1])?);
            let accion = Accion { estado: t[3].to_string(), escribe: simbolo(t[4])?, mov };
            if delta.insert(clave, accion).is_some() {
                return Err(ErrorFormato::NoDeterminista(n));
            }
        }
        Ok(Maquina { inicio: inicio.ok_or(ErrorFormato::SinInicio)?, aceptar, rechazar, blanco, delta })
    }

    /// Ejecuta sobre `entrada` hasta parar o hasta `max_pasos`. Si se pasa
    /// `traza`, se llama en cada paso con (paso, estado, cinta) antes de aplicar la transición.
    pub fn ejecutar(
        &self,
        entrada: &str,
        max_pasos: u64,
        mut traza: Option<&mut dyn FnMut(u64, &str, &Cinta)>,
    ) -> Ejecucion {
        let mut cinta = Cinta::nueva(entrada, self.blanco);
        let mut estado = self.inicio.clone();
        let mut pasos = 0;
        loop {
            if let Some(t) = traza.as_mut() {
                t(pasos, &estado, &cinta);
            }
            // Clave de búsqueda: clonamos el estado (String). Para máquinas
            // grandes convendría numerar estados, pero así el código es claro.
            let Some(acc) = self.delta.get(&(estado.clone(), cinta.leer())) else {
                let resultado = if self.aceptar.contains(&estado) {
                    Resultado::Acepta
                } else if self.rechazar.contains(&estado) {
                    Resultado::Rechaza
                } else {
                    Resultado::Detenida(estado)
                };
                return Ejecucion { resultado, pasos, cinta };
            };
            if pasos == max_pasos {
                return Ejecucion { resultado: Resultado::LimiteExcedido, pasos, cinta };
            }
            cinta.escribir_y_mover(acc.escribe, acc.mov);
            estado.clone_from(&acc.estado);
            pasos += 1;
        }
    }
}

/// Máquinas de ejemplo incluidas en el binario (también en `maquinas/*.tm`).
pub mod ejemplos {
    /// Suma 1 a un número binario.
    pub const INCREMENTO_BINARIO: &str = include_str!("../maquinas/incremento_binario.tm");
    /// Acepta palíndromos sobre {a, b}.
    pub const PALINDROMO: &str = include_str!("../maquinas/palindromo.tm");
    /// Castor afanoso de 3 estados y 2 símbolos.
    pub const CASTOR_3: &str = include_str!("../maquinas/castor3.tm");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errores_de_formato() {
        assert_eq!(Maquina::parsear("q0 1 -> q0 1 R").unwrap_err(), ErrorFormato::SinInicio);
        assert!(matches!(Maquina::parsear("inicio: a\na 1 -> b 1 X"), Err(ErrorFormato::Linea(2, _))));
        assert_eq!(
            Maquina::parsear("inicio: a\na 1 -> b 1 R\na 1 -> c 0 L").unwrap_err(),
            ErrorFormato::NoDeterminista(3)
        );
    }

    #[test]
    fn cinta_crece_por_la_izquierda() {
        let mut c = Cinta::nueva("1", '_');
        c.escribir_y_mover('1', Mov::L);
        c.escribir_y_mover('0', Mov::S);
        assert_eq!(c.contenido(), "01");
        assert_eq!(c.to_string(), "[0] 1 ");
    }

    #[test]
    fn bucle_infinito_alcanza_limite() {
        let m = Maquina::parsear("inicio: q\nq _ -> q _ R").unwrap();
        assert_eq!(m.ejecutar("", 1000, None).resultado, Resultado::LimiteExcedido);
    }
}
