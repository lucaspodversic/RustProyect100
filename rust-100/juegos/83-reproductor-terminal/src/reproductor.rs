//! Lógica del reproductor: máquina de estados + cola + modos.

use crate::audio::BackendAudio;
use crate::lista::Pista;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    Detenido,
    Reproduciendo,
    Pausado,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repetir {
    No,
    Todo,
    Una,
}

/// Comandos de alto nivel (la UI traduce teclas a esto).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Comando {
    PlayPausa,
    Detener,
    Siguiente,
    Anterior,
    Adelantar(u64),
    Retroceder(u64),
    SubirVolumen,
    BajarVolumen,
    Silencio,
    Aleatorio,
    CiclarRepetir,
    Elegir(usize),
}

impl Comando {
    /// Atajos de teclado estilo reproductores de consola.
    pub fn desde_tecla(c: char) -> Option<Comando> {
        Some(match c {
            ' ' | 'p' => Comando::PlayPausa,
            's' => Comando::Detener,
            'n' | '>' => Comando::Siguiente,
            'b' | '<' => Comando::Anterior,
            'l' => Comando::Adelantar(5_000),
            'h' => Comando::Retroceder(5_000),
            '+' | '=' => Comando::SubirVolumen,
            '-' => Comando::BajarVolumen,
            'm' => Comando::Silencio,
            'z' => Comando::Aleatorio,
            'r' => Comando::CiclarRepetir,
            _ => return None,
        })
    }
}

pub struct Reproductor<B: BackendAudio> {
    pub backend: B,
    pub pistas: Vec<Pista>,
    /// Orden de reproducción (índices a `pistas`); se baraja en aleatorio.
    pub orden: Vec<usize>,
    /// Posición dentro de `orden`.
    pub cursor: usize,
    pub estado: Estado,
    pub repetir: Repetir,
    pub aleatorio: bool,
    /// Volumen 0..=100 y mute independiente (al desmutear vuelve el nivel).
    pub volumen: u8,
    pub mute: bool,
    pub duracion_actual: u64,
    pub mensaje: String,
    rng: u64,
}

impl<B: BackendAudio> Reproductor<B> {
    pub fn new(backend: B, pistas: Vec<Pista>) -> Self {
        let orden = (0..pistas.len()).collect();
        Self { backend, pistas, orden, cursor: 0, estado: Estado::Detenido, repetir: Repetir::No, aleatorio: false, volumen: 70, mute: false, duracion_actual: 0, mensaje: String::new(), rng: 0x9E37_79B9 }
    }

    pub fn pista_actual(&self) -> Option<&Pista> {
        self.orden.get(self.cursor).map(|&i| &self.pistas[i])
    }

    pub fn posicion(&self) -> u64 {
        self.backend.posicion()
    }

    fn aplicar_volumen(&mut self) {
        let v = if self.mute { 0.0 } else { self.volumen as f32 / 100.0 };
        self.backend.volumen(v);
    }

    /// Carga la pista en `cursor` y la reproduce. Si un archivo falla, se
    /// salta al siguiente (como mucho una vuelta para no ciclar infinito).
    fn iniciar_actual(&mut self) {
        for _ in 0..self.orden.len() {
            let Some(p) = self.pista_actual() else { break };
            let ruta = p.ruta.clone();
            match self.backend.cargar(&ruta) {
                Ok(d) => {
                    self.duracion_actual = d;
                    self.aplicar_volumen();
                    self.backend.reproducir();
                    self.estado = Estado::Reproduciendo;
                    self.mensaje = format!("▶ {}", self.pista_actual().map(|p| p.titulo.as_str()).unwrap_or(""));
                    return;
                }
                Err(e) => {
                    self.mensaje = format!("⚠ {e}");
                    self.cursor = (self.cursor + 1) % self.orden.len();
                }
            }
        }
        self.estado = Estado::Detenido;
    }

    pub fn ejecutar(&mut self, c: Comando) {
        if self.pistas.is_empty() {
            self.mensaje = "Lista vacía".into();
            return;
        }
        match c {
            Comando::PlayPausa => match self.estado {
                Estado::Reproduciendo => {
                    self.backend.pausar();
                    self.estado = Estado::Pausado;
                }
                Estado::Pausado => {
                    self.backend.reproducir();
                    self.estado = Estado::Reproduciendo;
                }
                Estado::Detenido => self.iniciar_actual(),
            },
            Comando::Detener => {
                self.backend.detener();
                self.estado = Estado::Detenido;
            }
            Comando::Siguiente => self.avanzar_pista(true),
            Comando::Anterior => {
                // Convención habitual: si van más de 3 s, "anterior" reinicia la pista.
                if self.posicion() > 3_000 && self.estado != Estado::Detenido {
                    self.backend.buscar(0);
                } else {
                    self.cursor = if self.cursor == 0 { self.orden.len() - 1 } else { self.cursor - 1 };
                    self.iniciar_actual();
                }
            }
            Comando::Adelantar(ms) => {
                if self.estado != Estado::Detenido {
                    let p = (self.posicion() + ms).min(self.duracion_actual);
                    self.backend.buscar(p);
                }
            }
            Comando::Retroceder(ms) => {
                if self.estado != Estado::Detenido {
                    let p = self.posicion().saturating_sub(ms);
                    self.backend.buscar(p);
                }
            }
            Comando::SubirVolumen => {
                self.volumen = (self.volumen + 5).min(100);
                self.mute = false;
                self.aplicar_volumen();
            }
            Comando::BajarVolumen => {
                self.volumen = self.volumen.saturating_sub(5);
                self.aplicar_volumen();
            }
            Comando::Silencio => {
                self.mute = !self.mute;
                self.aplicar_volumen();
            }
            Comando::Aleatorio => self.alternar_aleatorio(),
            Comando::CiclarRepetir => {
                self.repetir = match self.repetir {
                    Repetir::No => Repetir::Todo,
                    Repetir::Todo => Repetir::Una,
                    Repetir::Una => Repetir::No,
                }
            }
            Comando::Elegir(i) => {
                if let Some(pos) = self.orden.iter().position(|&x| x == i) {
                    self.cursor = pos;
                    self.iniciar_actual();
                }
            }
        }
    }

    /// Pasa a la siguiente pista. `manual` = lo pidió el usuario (en ese
    /// caso "repetir una" no aplica, como en la mayoría de reproductores).
    fn avanzar_pista(&mut self, manual: bool) {
        if !manual && self.repetir == Repetir::Una {
            self.backend.buscar(0);
            self.backend.reproducir();
            return;
        }
        if self.cursor + 1 < self.orden.len() {
            self.cursor += 1;
        } else if self.repetir == Repetir::No && !manual {
            // Fin de la lista.
            self.backend.detener();
            self.estado = Estado::Detenido;
            self.cursor = 0;
            self.mensaje = "Fin de la lista".into();
            return;
        } else {
            self.cursor = 0;
        }
        self.iniciar_actual();
    }

    /// Baraja con Fisher–Yates dejando la pista actual primera; al
    /// desactivar vuelve al orden original sin cortar lo que suena.
    fn alternar_aleatorio(&mut self) {
        let actual = self.orden.get(self.cursor).copied().unwrap_or(0);
        self.aleatorio = !self.aleatorio;
        if self.aleatorio {
            let mut resto: Vec<usize> = (0..self.pistas.len()).filter(|&i| i != actual).collect();
            for i in (1..resto.len()).rev() {
                self.rng ^= self.rng << 13;
                self.rng ^= self.rng >> 7;
                self.rng ^= self.rng << 17;
                resto.swap(i, (self.rng % (i as u64 + 1)) as usize);
            }
            self.orden = std::iter::once(actual).chain(resto).collect();
            self.cursor = 0;
        } else {
            self.orden = (0..self.pistas.len()).collect();
            self.cursor = actual;
        }
    }

    /// Llamar periódicamente: avanza el reloj y detecta fin de pista.
    pub fn tick(&mut self, ms: u64) {
        if self.estado != Estado::Reproduciendo {
            return;
        }
        self.backend.avanzar(ms);
        if self.backend.termino() {
            self.avanzar_pista(false);
        }
    }
}
