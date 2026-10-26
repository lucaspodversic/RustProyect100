# 79 · Visualizador de audio

Convierte muestras de audio en **barras de espectro**, sin placa de sonido ni GPU.

- **FFT radix-2 propia** (Cooley–Tukey iterativa) verificada contra DFT y Parseval.
- **Ventana de Hann** y normalización de amplitud (un seno de 0.5 mide 0.5).
- **Bandas logarítmicas** 40 Hz–16 kHz, escala **dB** con rango dinámico configurable.
- **Suavizado** de vúmetro (ataque rápido / caída lenta) y **picos** que caen.
- **Fuentes mock**: suma de senos, chirp, ruido blanco, buffer en memoria y **WAV PCM16** (lectura/escritura, mono/estéreo).
- **Render**: barras Unicode en texto y imagen **PPM** con degradé.

```bash
cargo test                 # 10 tests
cargo run                  # chirp 100 Hz → 8 kHz
cargo run -- tema.wav      # analiza un WAV PCM16
```

## Limitaciones
No captura audio en vivo (con `cpal` bastaría alimentar un `Memoria`/ring buffer desde el callback). Sólo WAV PCM 16 bits.
