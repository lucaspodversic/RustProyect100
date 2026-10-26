//! Estado de la aplicación y manejo de teclas (sin I/O).
//!
//! La UI se modela como una pequeña máquina de estados: la vista actual, la
//! selección y el texto de búsqueda. `tecla()` recibe una tecla abstracta
//! ([`Tecla`]) para no depender de crossterm en la lógica → testeable sin TTY.

use crate::datos::{FuenteCadena, Resultado};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tecla {
    Arriba,
    Abajo,
    PagArriba,
    PagAbajo,
    Enter,
    Esc,
    Car(char),
    Borrar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vista {
    /// Lista de bloques (más nuevos arriba).
    Bloques,
    /// Detalle de un bloque, con su lista de transacciones.
    DetalleBloque { altura: u64, tx_sel: usize },
    /// Detalle de una transacción.
    DetalleTx { altura: u64, indice: usize },
    /// Historial de una dirección.
    Direccion(String),
}

pub struct App<F: FuenteCadena> {
    pub fuente: F,
    pub vista: Vista,
    /// Índice seleccionado en la lista de bloques (0 = el más nuevo).
    pub seleccion: usize,
    /// `Some(texto)` mientras se escribe una búsqueda (tecla `/`).
    pub busqueda: Option<String>,
    /// Mensaje de estado en la barra inferior.
    pub estado: String,
    pub salir: bool,
}

impl<F: FuenteCadena> App<F> {
    pub fn nueva(fuente: F) -> Self {
        Self { fuente, vista: Vista::Bloques, seleccion: 0, busqueda: None, estado: "↑↓ mover · Enter abrir · / buscar · a dirección · q salir".into(), salir: false }
    }

    /// Altura del bloque seleccionado en la lista.
    pub fn altura_seleccionada(&self) -> u64 {
        self.fuente.altura().saturating_sub(self.seleccion as u64)
    }

    pub fn tecla(&mut self, t: Tecla) {
        // Modo búsqueda: capturamos el texto hasta Enter/Esc.
        if let Some(q) = self.busqueda.as_mut() {
            match t {
                Tecla::Car(c) => q.push(c),
                Tecla::Borrar => {
                    q.pop();
                }
                Tecla::Esc => self.busqueda = None,
                Tecla::Enter => {
                    let q = self.busqueda.take().unwrap_or_default();
                    self.ejecutar_busqueda(&q);
                }
                _ => {}
            }
            return;
        }
        match (&self.vista, t) {
            (_, Tecla::Car('q')) => self.salir = true,
            (_, Tecla::Car('/')) => self.busqueda = Some(String::new()),
            (Vista::Bloques, Tecla::Arriba) => self.seleccion = self.seleccion.saturating_sub(1),
            (Vista::Bloques, Tecla::Abajo) => self.seleccion = (self.seleccion + 1).min(self.fuente.altura() as usize),
            (Vista::Bloques, Tecla::PagAbajo) => self.seleccion = (self.seleccion + 10).min(self.fuente.altura() as usize),
            (Vista::Bloques, Tecla::PagArriba) => self.seleccion = self.seleccion.saturating_sub(10),
            (Vista::Bloques, Tecla::Enter) => self.vista = Vista::DetalleBloque { altura: self.altura_seleccionada(), tx_sel: 0 },
            (Vista::DetalleBloque { altura, tx_sel }, t) => {
                let (altura, tx_sel) = (*altura, *tx_sel);
                let n = self.fuente.bloque(altura).map(|b| b.txs.len()).unwrap_or(0);
                match t {
                    Tecla::Arriba => self.vista = Vista::DetalleBloque { altura, tx_sel: tx_sel.saturating_sub(1) },
                    Tecla::Abajo => self.vista = Vista::DetalleBloque { altura, tx_sel: (tx_sel + 1).min(n.saturating_sub(1)) },
                    Tecla::Enter if n > 0 => self.vista = Vista::DetalleTx { altura, indice: tx_sel },
                    // ← / → navegan al bloque anterior / siguiente.
                    Tecla::Car('h') if altura > 0 => self.vista = Vista::DetalleBloque { altura: altura - 1, tx_sel: 0 },
                    Tecla::Car('l') if altura < self.fuente.altura() => self.vista = Vista::DetalleBloque { altura: altura + 1, tx_sel: 0 },
                    Tecla::Esc => self.vista = Vista::Bloques,
                    _ => {}
                }
            }
            (Vista::DetalleTx { altura, indice }, t) => {
                let (altura, indice) = (*altura, *indice);
                match t {
                    Tecla::Esc => self.vista = Vista::DetalleBloque { altura, tx_sel: indice },
                    Tecla::Car('a') => {
                        if let Some(tx) = self.fuente.bloque(altura).and_then(|b| b.txs.get(indice)) {
                            self.vista = Vista::Direccion(tx.de.clone());
                        }
                    }
                    _ => {}
                }
            }
            (Vista::Direccion(_), Tecla::Esc) => self.vista = Vista::Bloques,
            _ => {}
        }
    }

    /// Interpreta la consulta: número = altura; `@nombre` = dirección; si no, hash.
    pub fn ejecutar_busqueda(&mut self, q: &str) {
        let q = q.trim();
        if let Ok(h) = q.parse::<u64>() {
            if self.fuente.bloque(h).is_some() {
                self.vista = Vista::DetalleBloque { altura: h, tx_sel: 0 };
                self.estado = format!("Bloque #{h}");
            } else {
                self.estado = format!("No existe el bloque #{h}");
            }
            return;
        }
        if let Some(dir) = q.strip_prefix('@') {
            self.vista = Vista::Direccion(dir.to_string());
            self.estado = format!("Dirección {dir}");
            return;
        }
        match self.fuente.buscar_hash(q) {
            Some(Resultado::Bloque(h)) => {
                self.vista = Vista::DetalleBloque { altura: h, tx_sel: 0 };
                self.estado = format!("Encontrado bloque #{h}");
            }
            Some(Resultado::Tx { altura, indice }) => {
                self.vista = Vista::DetalleTx { altura, indice };
                self.estado = format!("Encontrada tx en bloque #{altura}");
            }
            None => self.estado = format!("Sin resultados para «{q}»"),
        }
    }
}
