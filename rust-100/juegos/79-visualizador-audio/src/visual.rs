//! Del espectro a barras: bandas logarítmicas, escala en dB, suavizado con
//! ataque rápido / caída lenta, marcadores de pico y render a texto o PPM.

use crate::fft::espectro;

pub struct Visualizador {
    pub tam_fft: usize,
    pub sr: u32,
    pub bandas: usize,
    /// Rango dinámico mostrado (dB): [piso, 0].
    pub piso_db: f32,
    /// Niveles actuales suavizados (0..1).
    pub niveles: Vec<f32>,
    /// Picos que caen lentamente.
    pub picos: Vec<f32>,
    pub ataque: f32,
    pub caida: f32,
    limites: Vec<(usize, usize)>,
}

impl Visualizador {
    pub fn new(sr: u32, tam_fft: usize, bandas: usize) -> Self {
        let limites = limites_log(sr, tam_fft, bandas, 40.0, (sr as f32 / 2.0).min(16_000.0));
        Self { tam_fft, sr, bandas, piso_db: -60.0, niveles: vec![0.0; bandas], picos: vec![0.0; bandas], ataque: 0.8, caida: 0.15, limites }
    }

    /// Frecuencia central (Hz) de cada banda.
    pub fn centros(&self) -> Vec<f32> {
        let res = self.sr as f32 / self.tam_fft as f32;
        self.limites.iter().map(|(a, b)| (*a + *b) as f32 / 2.0 * res).collect()
    }

    /// Procesa un bloque de `tam_fft` muestras y actualiza las barras.
    pub fn procesar(&mut self, bloque: &[f32]) {
        assert_eq!(bloque.len(), self.tam_fft);
        let mags = espectro(bloque);
        for (i, (a, b)) in self.limites.iter().enumerate() {
            // Tomamos el máximo de la banda (más legible que el promedio).
            let m = mags[*a..*b].iter().cloned().fold(0.0f32, f32::max);
            let db = 20.0 * m.max(1e-9).log10();
            let objetivo = ((db - self.piso_db) / -self.piso_db).clamp(0.0, 1.0);
            // Suavizado asimétrico: sube rápido, baja despacio (como un vúmetro).
            let k = if objetivo > self.niveles[i] { self.ataque } else { self.caida };
            self.niveles[i] += (objetivo - self.niveles[i]) * k;
            self.picos[i] = if self.niveles[i] >= self.picos[i] { self.niveles[i] } else { (self.picos[i] - 0.02).max(0.0) };
        }
    }

    /// Banda más alta en este momento.
    pub fn banda_dominante(&self) -> usize {
        self.niveles.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| i).unwrap_or(0)
    }

    /// Barras verticales en texto (`alto` filas). `█` nivel, `▔` pico.
    pub fn render_texto(&self, alto: usize) -> String {
        let mut s = String::new();
        for fila in (0..alto).rev() {
            let umbral = (fila as f32 + 0.5) / alto as f32;
            for i in 0..self.bandas {
                let pico_fila = ((self.picos[i] * alto as f32) as usize).min(alto - 1);
                s.push(if self.niveles[i] >= umbral {
                    '█'
                } else if fila == pico_fila && self.picos[i] > 0.0 {
                    '▔'
                } else {
                    ' '
                });
            }
            s.push('\n');
        }
        s
    }

    /// Imagen RGB (PPM P6) con degradé verde→amarillo→rojo según la altura.
    pub fn render_ppm(&self, ancho_barra: usize, alto: usize) -> Vec<u8> {
        let ancho = self.bandas * ancho_barra;
        let mut px = vec![10u8; ancho * alto * 3];
        for i in 0..self.bandas {
            let h = (self.niveles[i] * alto as f32) as usize;
            for y in 0..h {
                let t = y as f32 / alto as f32;
                let color = [(255.0 * (t * 2.0).min(1.0)) as u8, (255.0 * (2.0 - t * 2.0).min(1.0)) as u8, 40];
                for x in i * ancho_barra..(i + 1) * ancho_barra - 1 {
                    let idx = ((alto - 1 - y) * ancho + x) * 3;
                    px[idx..idx + 3].copy_from_slice(&color);
                }
            }
        }
        let mut out = format!("P6\n{ancho} {alto}\n255\n").into_bytes();
        out.extend(px);
        out
    }
}

/// Divide [f_min, f_max] en `n` bandas espaciadas logarítmicamente (como el
/// oído), convertidas a índices de bin y garantizando al menos 1 bin por banda.
pub fn limites_log(sr: u32, tam_fft: usize, n: usize, f_min: f32, f_max: f32) -> Vec<(usize, usize)> {
    let res = sr as f32 / tam_fft as f32;
    let max_bin = tam_fft / 2;
    let mut v = Vec::with_capacity(n);
    let mut prev = ((f_min / res) as usize).max(1);
    for i in 1..=n {
        let f = f_min * (f_max / f_min).powf(i as f32 / n as f32);
        let fin = ((f / res).round() as usize).clamp(prev + 1, max_bin);
        v.push((prev.min(max_bin - 1), fin));
        prev = fin;
    }
    v
}
