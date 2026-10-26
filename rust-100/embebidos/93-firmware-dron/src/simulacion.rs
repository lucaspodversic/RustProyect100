//! Simulador físico simplificado de un cuadricóptero en X.
//!
//! Solo modela la **dinámica rotacional**, que es lo que estabiliza el
//! controlador. La posición no se simula.
//!
//! - Cada motor tiene un retardo de primer orden (τ ≈ 30 ms): el empuje no
//!   cambia al instante porque la hélice tiene inercia.
//! - Par de roll = brazo × (empuje izquierdo − empuje derecho). Lo mismo para
//!   pitch con delanteros − traseros. El yaw usa la diferencia de par
//!   aerodinámico entre hélices CW y CCW.
//! - Aceleración angular = par / inercia, integrada con Euler semi‑implícito.
//! - El IMU devuelve gravedad rotada y velocidad angular, con ruido y bias
//!   (generador xorshift: determinista y sin dependencias).

use libm::{cosf, sinf};

use crate::actitud::Imu;
use crate::mezclador::TABLA_X;

/// Parámetros físicos.
#[derive(Debug, Clone, Copy)]
pub struct Fisica {
    /// Empuje máximo por motor (N).
    pub empuje_max: f32,
    /// Brazo efectivo en el eje roll/pitch (m).
    pub brazo: f32,
    /// Momento de inercia roll/pitch (kg·m²).
    pub inercia: f32,
    /// Inercia yaw.
    pub inercia_yaw: f32,
    /// Coeficiente de par de yaw por N de empuje (m).
    pub k_yaw: f32,
    /// Constante de tiempo del motor (s).
    pub tau_motor: f32,
    /// Amortiguamiento aerodinámico (1/s).
    pub amortiguamiento: f32,
    /// Ruido del giroscopio (°/s, amplitud).
    pub ruido_giro: f32,
    /// Ruido del acelerómetro (g, amplitud): vibraciones.
    pub ruido_acel: f32,
    /// Bias del giroscopio (°/s).
    pub bias_giro: [f32; 3],
}

impl Default for Fisica {
    fn default() -> Self {
        Self {
            empuje_max: 4.0,
            brazo: 0.08,
            inercia: 0.004,
            inercia_yaw: 0.007,
            k_yaw: 0.015,
            tau_motor: 0.03,
            amortiguamiento: 0.5,
            ruido_giro: 1.0,
            ruido_acel: 0.05,
            bias_giro: [0.8, -0.5, 0.3],
        }
    }
}

/// Estado del dron simulado.
#[derive(Debug, Clone, Copy)]
pub struct Dron {
    /// Parámetros.
    pub f: Fisica,
    /// Ángulos reales (°): roll, pitch, yaw.
    pub angulo: [f32; 3],
    /// Velocidades reales (°/s).
    pub velocidad: [f32; 3],
    /// Empuje real de cada motor (0..1), tras el retardo.
    pub motores: [f32; 4],
    semilla: u32,
}

impl Dron {
    /// Crea el dron con una actitud inicial.
    pub fn new(f: Fisica, roll: f32, pitch: f32) -> Self {
        Self { f, angulo: [roll, pitch, 0.0], velocidad: [0.0; 3], motores: [0.0; 4], semilla: 0x1234_5678 }
    }

    /// Ruido uniforme en [−1, 1] (xorshift32).
    fn ruido(&mut self) -> f32 {
        let mut x = self.semilla;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.semilla = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Lectura del IMU para el estado actual.
    pub fn imu(&mut self) -> Imu {
        let (r, p) = (self.angulo[0].to_radians(), self.angulo[1].to_radians());
        // Gravedad en ejes del cuerpo (rotación roll‑pitch), Z arriba.
        let acel = [
            -sinf(p) + self.f.ruido_acel * self.ruido(),
            sinf(r) * cosf(p) + self.f.ruido_acel * self.ruido(),
            cosf(r) * cosf(p) + self.f.ruido_acel * self.ruido(),
        ];
        let mut giro = [0.0; 3];
        for (i, g) in giro.iter_mut().enumerate() {
            *g = self.velocidad[i] + self.f.bias_giro[i] + self.f.ruido_giro * self.ruido();
        }
        Imu { acel, giro }
    }

    /// Integra `dt` segundos con los comandos de motor dados.
    pub fn paso(&mut self, comandos: [f32; 4], dt: f32) {
        let k = dt / (self.f.tau_motor + dt);
        for (m, c) in self.motores.iter_mut().zip(comandos) {
            *m += (c - *m) * k;
        }
        // Par = suma de (empuje × signo de la tabla de mezcla): la tabla
        // describe exactamente qué motores generan cada par.
        let mut par = [0.0f32; 3];
        for (m, fila) in self.motores.iter().zip(TABLA_X.iter()) {
            let empuje = m * self.f.empuje_max;
            par[0] += fila[0] * empuje * self.f.brazo;
            par[1] += fila[1] * empuje * self.f.brazo;
            par[2] += fila[2] * empuje * self.f.k_yaw;
        }
        let inercias = [self.f.inercia, self.f.inercia, self.f.inercia_yaw];
        for i in 0..3 {
            let acel_ang = (par[i] / inercias[i]).to_degrees() - self.f.amortiguamiento * self.velocidad[i];
            self.velocidad[i] += acel_ang * dt;
            self.angulo[i] += self.velocidad[i] * dt;
        }
    }
}
