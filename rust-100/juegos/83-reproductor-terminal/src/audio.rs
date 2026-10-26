//! Abstracción del dispositivo de audio.
//!
//! El reproductor sólo habla con el trait [`BackendAudio`]. En producción
//! se implementaría con `rodio`/`cpal`; acá incluimos [`BackendSimulado`],
//! un "reloj" que avanza cuando se le pide y registra cada llamada, lo que
//! permite testear todo sin placa de sonido.

#[derive(Debug, Clone, PartialEq)]
pub enum Llamada {
    Cargar(String),
    Reproducir,
    Pausar,
    Detener,
    Buscar(u64),
    Volumen(f32),
}

pub trait BackendAudio {
    /// Abre un archivo; devuelve su duración en ms o un error legible.
    fn cargar(&mut self, ruta: &str) -> Result<u64, String>;
    fn reproducir(&mut self);
    fn pausar(&mut self);
    fn detener(&mut self);
    fn buscar(&mut self, ms: u64);
    /// Volumen lineal 0.0..=1.0.
    fn volumen(&mut self, v: f32);
    /// Posición actual en ms.
    fn posicion(&self) -> u64;
    /// ¿Terminó la pista actual?
    fn termino(&self) -> bool;
    /// Avanza el tiempo (en un backend real no hace nada: el tiempo corre solo).
    fn avanzar(&mut self, _ms: u64) {}
}

/// Backend simulado y determinista.
#[derive(Debug, Default)]
pub struct BackendSimulado {
    pub llamadas: Vec<Llamada>,
    /// Duraciones por ruta; las rutas ausentes fallan al cargar.
    pub catalogo: std::collections::HashMap<String, u64>,
    duracion: u64,
    pos: u64,
    sonando: bool,
    pub vol: f32,
}

impl BackendSimulado {
    pub fn con_catalogo(items: &[(&str, u64)]) -> Self {
        Self { catalogo: items.iter().map(|(r, d)| (r.to_string(), *d)).collect(), vol: 1.0, ..Default::default() }
    }
    pub fn sonando(&self) -> bool {
        self.sonando
    }
}

impl BackendAudio for BackendSimulado {
    fn cargar(&mut self, ruta: &str) -> Result<u64, String> {
        self.llamadas.push(Llamada::Cargar(ruta.into()));
        let d = *self.catalogo.get(ruta).ok_or_else(|| format!("no se puede abrir {ruta}"))?;
        self.duracion = d;
        self.pos = 0;
        self.sonando = false;
        Ok(d)
    }
    fn reproducir(&mut self) {
        self.llamadas.push(Llamada::Reproducir);
        self.sonando = true;
    }
    fn pausar(&mut self) {
        self.llamadas.push(Llamada::Pausar);
        self.sonando = false;
    }
    fn detener(&mut self) {
        self.llamadas.push(Llamada::Detener);
        self.sonando = false;
        self.pos = 0;
    }
    fn buscar(&mut self, ms: u64) {
        self.llamadas.push(Llamada::Buscar(ms));
        self.pos = ms.min(self.duracion);
    }
    fn volumen(&mut self, v: f32) {
        self.llamadas.push(Llamada::Volumen(v));
        self.vol = v;
    }
    fn posicion(&self) -> u64 {
        self.pos
    }
    fn termino(&self) -> bool {
        self.duracion > 0 && self.pos >= self.duracion
    }
    fn avanzar(&mut self, ms: u64) {
        if self.sonando {
            self.pos = (self.pos + ms).min(self.duracion);
        }
    }
}
