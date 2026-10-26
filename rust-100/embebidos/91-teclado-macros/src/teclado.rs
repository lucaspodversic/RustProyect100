//! Integración: matriz → antirrebote → capas/macros → reporte HID.

use embedded_hal::digital::{InputPin, OutputPin};

use crate::antirrebote::{Antirrebote, Evento};
use crate::hid::Reporte;
use crate::macros::{Ejecutor, Paso};
use crate::mapa::{resolver, Accion, Mapa, COLUMNAS, FILAS};
use crate::matriz::Matriz;

/// Firmware del teclado.
pub struct Teclado<O, I> {
    matriz: Matriz<O, I, FILAS, COLUMNAS>,
    filtro: Antirrebote<FILAS, COLUMNAS>,
    mapa: Mapa,
    macros: &'static [&'static [Paso]],
    /// Acción "congelada" de cada tecla pulsada: se resuelve al pulsar, de
    /// modo que soltar Fn antes que la tecla no deja una tecla "pegada".
    activas: [[Option<Accion>; COLUMNAS]; FILAS],
    capa_momentanea: u8,
    capa_alternada: u8,
    macro_en_curso: Option<Ejecutor>,
    ultimo: Reporte,
}

impl<O: OutputPin, I: InputPin> Teclado<O, I> {
    /// Crea el teclado.
    pub fn new(matriz: Matriz<O, I, FILAS, COLUMNAS>, mapa: Mapa, macros: &'static [&'static [Paso]]) -> Self {
        Self {
            matriz,
            filtro: Antirrebote::default(),
            mapa,
            macros,
            activas: [[None; COLUMNAS]; FILAS],
            capa_momentanea: 0,
            capa_alternada: 0,
            macro_en_curso: None,
            ultimo: Reporte::default(),
        }
    }

    /// Máscara de capas activas (la 0 siempre lo está).
    pub fn capas(&self) -> u8 {
        1 | self.capa_momentanea | self.capa_alternada
    }

    /// Un tick (~1 ms): escanea, filtra y devuelve un reporte **si cambió**
    /// respecto al último enviado (se ahorra tráfico USB).
    pub fn tick(&mut self) -> Option<Reporte> {
        let crudo = self.matriz.escanear(|| {});
        let mut eventos: [Option<Evento>; FILAS * COLUMNAS] = [None; FILAS * COLUMNAS];
        let mut n = 0;
        self.filtro.actualizar(&crudo, |e| {
            eventos[n] = Some(e);
            n += 1;
        });
        for e in eventos.iter().flatten() {
            self.procesar(*e);
        }

        // Una macro en curso tiene prioridad sobre el resto de teclas.
        let reporte = if let Some(ej) = self.macro_en_curso.as_mut() {
            match ej.siguiente() {
                Some(r) => r,
                None => {
                    self.macro_en_curso = None;
                    self.reporte_normal()
                }
            }
        } else {
            self.reporte_normal()
        };
        if reporte != self.ultimo {
            self.ultimo = reporte;
            Some(reporte)
        } else {
            None
        }
    }

    fn procesar(&mut self, e: Evento) {
        match e {
            Evento::Pulsada(f, c) => {
                let (f, c) = (f as usize, c as usize);
                let a = resolver(&self.mapa, self.capas(), f, c);
                match a {
                    Accion::CapaMomentanea(n) => self.capa_momentanea |= 1 << n,
                    Accion::CapaAlternar(n) => self.capa_alternada ^= 1 << n,
                    Accion::Macro(i) => {
                        if let Some(p) = self.macros.get(i as usize) {
                            self.macro_en_curso = Some(Ejecutor::new(p));
                        }
                    }
                    _ => {}
                }
                self.activas[f][c] = Some(a);
            }
            Evento::Soltada(f, c) => {
                let (f, c) = (f as usize, c as usize);
                if let Some(Accion::CapaMomentanea(n)) = self.activas[f][c] {
                    self.capa_momentanea &= !(1 << n);
                }
                self.activas[f][c] = None;
            }
        }
    }

    fn reporte_normal(&self) -> Reporte {
        let mut r = Reporte::default();
        for a in self.activas.iter().flatten().flatten() {
            if let Accion::Tecla(codigo, m) = *a {
                r.modificadores |= m;
                if codigo != 0 {
                    r.agregar(codigo);
                }
            }
        }
        r
    }
}
