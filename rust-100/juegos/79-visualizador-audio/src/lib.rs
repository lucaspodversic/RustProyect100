//! # Visualizador de audio (proyecto 79)
//!
//! - [`fft`]: FFT radix-2 propia + ventana de Hann.
//! - [`fuentes`]: audio simulado (senos, chirp, ruido) y WAV PCM16.
//! - [`visual`]: bandas log, dB, suavizado, picos y render texto/PPM.
//!
//! No usa la placa de sonido: cualquier fuente implementa el trait `Fuente`
//! (en un frontend real, el callback de `cpal` llenaría un `Memoria`/ring buffer).

pub mod fft;
pub mod fuentes;
pub mod visual;
