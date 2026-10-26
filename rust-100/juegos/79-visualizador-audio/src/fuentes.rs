//! Fuentes de audio **simuladas** (no hace falta placa de sonido) y un
//! lector/escritor WAV PCM16 mínimo para analizar archivos reales.

use std::f32::consts::PI;

/// Una fuente entrega bloques de muestras mono en [-1, 1].
pub trait Fuente {
    fn frecuencia_muestreo(&self) -> u32;
    /// Llena `buf`; devuelve cuántas muestras escribió (0 = terminó).
    fn leer(&mut self, buf: &mut [f32]) -> usize;
}

/// Suma de senoidales (acordes, tonos de prueba).
pub struct Senos {
    pub sr: u32,
    pub tonos: Vec<(f32, f32)>, // (frecuencia Hz, amplitud)
    t: u64,
}

impl Senos {
    pub fn new(sr: u32, tonos: Vec<(f32, f32)>) -> Self {
        Self { sr, tonos, t: 0 }
    }
}

impl Fuente for Senos {
    fn frecuencia_muestreo(&self) -> u32 {
        self.sr
    }
    fn leer(&mut self, buf: &mut [f32]) -> usize {
        for s in buf.iter_mut() {
            let t = self.t as f32 / self.sr as f32;
            *s = self.tonos.iter().map(|(f, a)| a * (2.0 * PI * f * t).sin()).sum();
            self.t += 1;
        }
        buf.len()
    }
}

/// Barrido de frecuencia lineal (chirp) de `f0` a `f1` en `dur` segundos.
pub struct Chirp {
    pub sr: u32,
    pub f0: f32,
    pub f1: f32,
    pub dur: f32,
    t: u64,
    fase: f32,
}

impl Chirp {
    pub fn new(sr: u32, f0: f32, f1: f32, dur: f32) -> Self {
        Self { sr, f0, f1, dur, t: 0, fase: 0.0 }
    }
}

impl Fuente for Chirp {
    fn frecuencia_muestreo(&self) -> u32 {
        self.sr
    }
    fn leer(&mut self, buf: &mut [f32]) -> usize {
        let total = (self.dur * self.sr as f32) as u64;
        let mut n = 0;
        for s in buf.iter_mut() {
            if self.t >= total {
                break;
            }
            let f = self.f0 + (self.f1 - self.f0) * (self.t as f32 / total as f32);
            // Integramos la fase para que no haya saltos al cambiar la frecuencia.
            self.fase = (self.fase + 2.0 * PI * f / self.sr as f32) % (2.0 * PI);
            *s = self.fase.sin() * 0.8;
            self.t += 1;
            n += 1;
        }
        n
    }
}

/// Ruido blanco determinista (xorshift).
pub struct Ruido {
    pub sr: u32,
    estado: u32,
}

impl Ruido {
    pub fn new(sr: u32, semilla: u32) -> Self {
        Self { sr, estado: semilla.max(1) }
    }
}

impl Fuente for Ruido {
    fn frecuencia_muestreo(&self) -> u32 {
        self.sr
    }
    fn leer(&mut self, buf: &mut [f32]) -> usize {
        for s in buf.iter_mut() {
            self.estado ^= self.estado << 13;
            self.estado ^= self.estado >> 17;
            self.estado ^= self.estado << 5;
            *s = (self.estado as f32 / u32::MAX as f32) * 2.0 - 1.0;
        }
        buf.len()
    }
}

/// Muestras ya en memoria (p. ej. leídas de un WAV).
pub struct Memoria {
    pub sr: u32,
    pub datos: Vec<f32>,
    pos: usize,
}

impl Memoria {
    pub fn new(sr: u32, datos: Vec<f32>) -> Self {
        Self { sr, datos, pos: 0 }
    }
}

impl Fuente for Memoria {
    fn frecuencia_muestreo(&self) -> u32 {
        self.sr
    }
    fn leer(&mut self, buf: &mut [f32]) -> usize {
        let n = buf.len().min(self.datos.len() - self.pos);
        buf[..n].copy_from_slice(&self.datos[self.pos..self.pos + n]);
        self.pos += n;
        n
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ErrorWav {
    Formato(&'static str),
}

/// Escribe WAV PCM 16 bits mono.
pub fn escribir_wav(sr: u32, muestras: &[f32]) -> Vec<u8> {
    let datos = muestras.len() as u32 * 2;
    let mut v = Vec::with_capacity(44 + datos as usize);
    v.extend(b"RIFF");
    v.extend((36 + datos).to_le_bytes());
    v.extend(b"WAVEfmt ");
    v.extend(16u32.to_le_bytes()); // tamaño del chunk fmt
    v.extend(1u16.to_le_bytes()); // PCM
    v.extend(1u16.to_le_bytes()); // mono
    v.extend(sr.to_le_bytes());
    v.extend((sr * 2).to_le_bytes()); // byte rate
    v.extend(2u16.to_le_bytes()); // block align
    v.extend(16u16.to_le_bytes()); // bits
    v.extend(b"data");
    v.extend(datos.to_le_bytes());
    for s in muestras {
        v.extend(((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    v
}

/// Lee WAV PCM16 (mono o estéreo → mezcla a mono). Recorre los chunks
/// para tolerar chunks extra (LIST, etc.).
pub fn leer_wav(b: &[u8]) -> Result<(u32, Vec<f32>), ErrorWav> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err(ErrorWav::Formato("no es RIFF/WAVE"));
    }
    let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
    let (mut i, mut fmt) = (12, None);
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let tam = u32_at(i + 4) as usize;
        let cuerpo = i + 8;
        if cuerpo + tam > b.len() {
            return Err(ErrorWav::Formato("chunk truncado"));
        }
        if id == b"fmt " {
            if tam < 16 || u16_at(cuerpo) != 1 || u16_at(cuerpo + 14) != 16 {
                return Err(ErrorWav::Formato("sólo PCM 16 bits"));
            }
            fmt = Some((u16_at(cuerpo + 2) as usize, u32_at(cuerpo + 4)));
        } else if id == b"data" {
            let (canales, sr) = fmt.ok_or(ErrorWav::Formato("data antes de fmt"))?;
            if canales == 0 {
                return Err(ErrorWav::Formato("0 canales"));
            }
            let m: Vec<f32> = b[cuerpo..cuerpo + tam]
                .chunks_exact(2 * canales)
                .map(|f| f.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).sum::<f32>() / canales as f32)
                .collect();
            return Ok((sr, m));
        }
        i = cuerpo + tam + (tam & 1); // los chunks se alinean a 2 bytes
    }
    Err(ErrorWav::Formato("sin chunk data"))
}
