//! Mini-motor 2D headless: vectores, rectángulos (AABB), RNG determinista y
//! un framebuffer de caracteres + color que sirve para tests y para la terminal.

/// Vector 2D en unidades del mundo (1 unidad = 1 celda del framebuffer).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl std::ops::Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, k: f32) -> V2 {
        V2::new(self.x * k, self.y * k)
    }
}

/// Caja alineada a los ejes: posición = esquina superior izquierda.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub pos: V2,
    pub tam: V2,
}

impl Aabb {
    /// Intersección estricta (tocarse en el borde no cuenta).
    pub fn choca(&self, o: &Aabb) -> bool {
        self.pos.x < o.pos.x + o.tam.x && o.pos.x < self.pos.x + self.tam.x && self.pos.y < o.pos.y + o.tam.y && o.pos.y < self.pos.y + self.tam.y
    }
}

/// xorshift64*: rápido, determinista y reproducible en tests (no criptográfico).
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(semilla: u64) -> Self {
        Self(semilla.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Flotante uniforme en [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn rango(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }
}

/// Framebuffer de celdas (carácter + color RGB). Es el "render target".
#[derive(Debug, Clone, PartialEq)]
pub struct Framebuffer {
    pub ancho: usize,
    pub alto: usize,
    pub celdas: Vec<(char, [u8; 3])>,
}

impl Framebuffer {
    pub fn new(ancho: usize, alto: usize) -> Self {
        Self { ancho, alto, celdas: vec![(' ', [0, 0, 0]); ancho * alto] }
    }
    pub fn limpiar(&mut self) {
        self.celdas.fill((' ', [0, 0, 0]));
    }
    /// Dibuja un carácter; ignora coordenadas fuera de pantalla (clipping).
    pub fn poner(&mut self, x: i32, y: i32, c: char, color: [u8; 3]) {
        if x >= 0 && y >= 0 && (x as usize) < self.ancho && (y as usize) < self.alto {
            self.celdas[y as usize * self.ancho + x as usize] = (c, color);
        }
    }
    pub fn texto(&mut self, x: i32, y: i32, s: &str, color: [u8; 3]) {
        for (i, c) in s.chars().enumerate() {
            self.poner(x + i as i32, y, c, color);
        }
    }
    pub fn get(&self, x: usize, y: usize) -> char {
        self.celdas[y * self.ancho + x].0
    }
    /// Volcado a texto plano (una línea por fila).
    pub fn a_texto(&self) -> String {
        self.celdas.chunks(self.ancho).map(|f| f.iter().map(|c| c.0).collect::<String>()).collect::<Vec<_>>().join("\n")
    }
    /// Volcado con colores ANSI truecolor para la terminal.
    pub fn a_ansi(&self) -> String {
        let mut s = String::new();
        for fila in self.celdas.chunks(self.ancho) {
            for (c, [r, g, b]) in fila {
                s += &format!("\x1b[38;2;{r};{g};{b}m{c}");
            }
            s += "\x1b[0m\n";
        }
        s
    }
}
