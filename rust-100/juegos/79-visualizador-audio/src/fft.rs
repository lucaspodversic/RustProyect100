//! FFT radix-2 iterativa (Cooley–Tukey) implementada desde cero.
//!
//! Pasos: 1) reordenar por inversión de bits, 2) "mariposas" en log2(N)
//! etapas. Complejidad O(N log N) frente a O(N²) de la DFT directa.

use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Complejo {
    pub re: f32,
    pub im: f32,
}

impl Complejo {
    pub fn new(re: f32, im: f32) -> Self {
        Self { re, im }
    }
    pub fn modulo(&self) -> f32 {
        (self.re * self.re + self.im * self.im).sqrt()
    }
    fn mul(self, o: Self) -> Self {
        Self::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
    fn add(self, o: Self) -> Self {
        Self::new(self.re + o.re, self.im + o.im)
    }
    fn sub(self, o: Self) -> Self {
        Self::new(self.re - o.re, self.im - o.im)
    }
}

/// FFT in-place. `datos.len()` debe ser potencia de 2.
pub fn fft(datos: &mut [Complejo]) {
    let n = datos.len();
    assert!(n.is_power_of_two(), "la FFT radix-2 requiere N potencia de 2");
    // 1) Permutación por inversión de bits.
    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = if bits == 0 { 0 } else { i.reverse_bits() >> (usize::BITS - bits) };
        if j > i {
            datos.swap(i, j);
        }
    }
    // 2) Etapas de mariposas: tamaño 2, 4, 8, ...
    let mut tam = 2;
    while tam <= n {
        let ang = -2.0 * PI / tam as f32;
        let w_paso = Complejo::new(ang.cos(), ang.sin());
        for inicio in (0..n).step_by(tam) {
            let mut w = Complejo::new(1.0, 0.0);
            for k in 0..tam / 2 {
                let a = datos[inicio + k];
                let b = datos[inicio + k + tam / 2].mul(w);
                datos[inicio + k] = a.add(b);
                datos[inicio + k + tam / 2] = a.sub(b);
                w = w.mul(w_paso);
            }
        }
        tam *= 2;
    }
}

/// DFT directa O(N²) — sólo para verificar la FFT en tests.
pub fn dft(x: &[Complejo]) -> Vec<Complejo> {
    let n = x.len();
    (0..n)
        .map(|k| {
            x.iter().enumerate().fold(Complejo::default(), |acc, (t, v)| {
                let a = -2.0 * PI * (k * t) as f32 / n as f32;
                acc.add(v.mul(Complejo::new(a.cos(), a.sin())))
            })
        })
        .collect()
}

/// Ventana de Hann: reduce la "fuga espectral" al cortar la señal en bloques.
pub fn hann(n: usize) -> Vec<f32> {
    if n == 1 {
        return vec![1.0];
    }
    (0..n).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / (n - 1) as f32).cos()).collect()
}

/// Espectro de magnitudes (sólo la mitad útil: 0..N/2) de una señal real,
/// normalizado para que un seno de amplitud 1 dé ≈1 en su bin.
pub fn espectro(muestras: &[f32]) -> Vec<f32> {
    let n = muestras.len();
    let w = hann(n);
    let suma_w: f32 = w.iter().sum();
    let mut buf: Vec<Complejo> = muestras.iter().zip(&w).map(|(s, w)| Complejo::new(s * w, 0.0)).collect();
    fft(&mut buf);
    buf[..n / 2].iter().map(|c| 2.0 * c.modulo() / suma_w).collect()
}
