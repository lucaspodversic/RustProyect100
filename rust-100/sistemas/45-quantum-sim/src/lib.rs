//! # Simulador cuántico básico
//!
//! Estado de `n` qubits = vector de 2^n amplitudes complejas.
//! Compuertas: I, X, Z, H, CNOT. Medición colapsa el estado.

use std::f64::consts::FRAC_1_SQRT_2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Complex = Complex { re: 0.0, im: 0.0 };
    pub const ONE: Complex = Complex { re: 1.0, im: 0.0 };
    pub fn new(re: f64, im: f64) -> Self { Self { re, im } }
    pub fn norm_sq(self) -> f64 { self.re * self.re + self.im * self.im }
    pub fn mul(self, o: Complex) -> Complex {
        Complex { re: self.re * o.re - self.im * o.im, im: self.re * o.im + self.im * o.re }
    }
    pub fn add(self, o: Complex) -> Complex { Complex { re: self.re + o.re, im: self.im + o.im } }
    pub fn scale(self, s: f64) -> Complex { Complex { re: self.re * s, im: self.im * s } }
}

/// Estado puro de `n_qubits`.
#[derive(Debug, Clone)]
pub struct State {
    pub n: usize,
    pub amp: Vec<Complex>,
}

impl State {
    /// |00…0⟩
    pub fn zeros(n: usize) -> Self {
        let mut amp = vec![Complex::ZERO; 1 << n];
        amp[0] = Complex::ONE;
        Self { n, amp }
    }

    /// Aplica una compuerta 2×2 al qubit `q` (0 = menos significativo).
    pub fn apply_1(&mut self, q: usize, gate: [[Complex; 2]; 2]) {
        let bit = 1 << q;
        let mut new = self.amp.clone();
        for i in 0..self.amp.len() {
            if i & bit == 0 {
                let j = i | bit;
                let a0 = self.amp[i];
                let a1 = self.amp[j];
                new[i] = gate[0][0].mul(a0).add(gate[0][1].mul(a1));
                new[j] = gate[1][0].mul(a0).add(gate[1][1].mul(a1));
            }
        }
        self.amp = new;
    }

    pub fn x(&mut self, q: usize) {
        self.apply_1(q, [[Complex::ZERO, Complex::ONE], [Complex::ONE, Complex::ZERO]]);
    }
    pub fn z(&mut self, q: usize) {
        self.apply_1(q, [[Complex::ONE, Complex::ZERO], [Complex::ZERO, Complex::new(-1.0, 0.0)]]);
    }
    pub fn h(&mut self, q: usize) {
        let s = FRAC_1_SQRT_2;
        self.apply_1(
            q,
            [
                [Complex::new(s, 0.0), Complex::new(s, 0.0)],
                [Complex::new(s, 0.0), Complex::new(-s, 0.0)],
            ],
        );
    }

    /// CNOT: control → target.
    pub fn cnot(&mut self, control: usize, target: usize) {
        let cb = 1 << control;
        let tb = 1 << target;
        let mut new = self.amp.clone();
        for i in 0..self.amp.len() {
            if i & cb != 0 {
                let j = i ^ tb;
                if i < j {
                    new.swap(i, j);
                }
            }
        }
        // swap above is wrong for clone-then-swap; do proper:
        let mut new = self.amp.clone();
        for i in 0..self.amp.len() {
            if i & cb != 0 && i & tb == 0 {
                let j = i | tb;
                new.swap(i, j);
            }
        }
        self.amp = new;
    }

    /// Probabilidad de medir 1 en el qubit `q`.
    pub fn prob_one(&self, q: usize) -> f64 {
        let bit = 1 << q;
        self.amp.iter().enumerate().filter(|(i, _)| i & bit != 0).map(|(_, a)| a.norm_sq()).sum()
    }

    /// Mide el qubit `q` (colapso). Usa un RNG simple LCG pasado por ref.
    pub fn measure(&mut self, q: usize, rng: &mut Lcg) -> u8 {
        let p1 = self.prob_one(q);
        let outcome = if rng.next_f64() < p1 { 1 } else { 0 };
        let bit = 1 << q;
        let mut norm = 0.0;
        for (i, a) in self.amp.iter_mut().enumerate() {
            let bit_set = (i & bit) != 0;
            if (outcome == 1) != bit_set {
                *a = Complex::ZERO;
            } else {
                norm += a.norm_sq();
            }
        }
        let inv = 1.0 / norm.sqrt();
        for a in &mut self.amp {
            *a = a.scale(inv);
        }
        outcome
    }
}

/// LCG reproducible para tests.
pub struct Lcg(u64);
impl Lcg {
    pub fn new(seed: u64) -> Self { Self(seed) }
    pub fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.0 >> 33) as f64 / (1u64 << 31) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hadamard_y_medicion() {
        let mut s = State::zeros(1);
        s.h(0);
        assert!((s.prob_one(0) - 0.5).abs() < 1e-9);
        // Tras medir, estado colapsado.
        let mut rng = Lcg::new(42);
        let _ = s.measure(0, &mut rng);
        let p = s.prob_one(0);
        assert!(p < 1e-9 || (p - 1.0).abs() < 1e-9);
    }
    #[test]
    fn bell_pair() {
        let mut s = State::zeros(2);
        s.h(0);
        s.cnot(0, 1);
        // (|00⟩+|11⟩)/√2 → prob de qubit1=1 == prob qubit0=1 == 0.5, correlacionados.
        assert!((s.prob_one(0) - 0.5).abs() < 1e-9);
        assert!((s.prob_one(1) - 0.5).abs() < 1e-9);
        let mut rng = Lcg::new(7);
        let a = s.measure(0, &mut rng);
        let b = s.measure(1, &mut rng);
        assert_eq!(a, b, "par de Bell perfectamente correlacionado");
    }
    #[test]
    fn x_flip() {
        let mut s = State::zeros(1);
        s.x(0);
        assert!((s.prob_one(0) - 1.0).abs() < 1e-9);
    }
}
