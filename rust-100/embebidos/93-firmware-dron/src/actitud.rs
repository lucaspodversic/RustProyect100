//! Estimación de actitud con **filtro complementario**.
//!
//! - El **giroscopio** responde rápido, pero al integrarlo el error crece sin
//!   límite (deriva).
//! - El **acelerómetro** mide la gravedad, así que da un ángulo absoluto,
//!   pero es muy ruidoso (vibraciones) y se altera con las aceleraciones.
//!
//! El filtro combina ambos, confiando en el giro a corto plazo y en el
//! acelerómetro a largo plazo:
//!
//! `ángulo = α·(ángulo + giro·dt) + (1−α)·ángulo_acel`
//!
//! Con α = 0,98 y dt = 1 ms, la constante de tiempo es τ = α·dt/(1−α) ≈ 49 ms.

use libm::{atan2f, sqrtf};

/// Lectura del IMU en unidades físicas.
#[derive(Debug, Clone, Copy, Default)]
pub struct Imu {
    /// Aceleración (en g) en ejes del cuerpo: X adelante, Y derecha y Z hacia
    /// **arriba** (convención elegida por simplicidad): en reposo `[0, 0, 1]`.
    pub acel: [f32; 3],
    /// Velocidad angular (°/s): [roll, pitch, yaw].
    pub giro: [f32; 3],
}

/// Ángulos de roll y pitch (grados) según el acelerómetro.
pub fn angulos_acel(a: [f32; 3]) -> (f32, f32) {
    let roll = atan2f(a[1], a[2]).to_degrees();
    let pitch = atan2f(-a[0], sqrtf(a[1] * a[1] + a[2] * a[2])).to_degrees();
    (roll, pitch)
}

/// Filtro complementario.
#[derive(Debug, Clone, Copy)]
pub struct Complementario {
    /// Peso del giroscopio (0..1).
    pub alfa: f32,
    /// Roll estimado (°).
    pub roll: f32,
    /// Pitch estimado (°).
    pub pitch: f32,
    iniciado: bool,
}

impl Complementario {
    /// Crea el filtro.
    pub const fn new(alfa: f32) -> Self {
        Self { alfa, roll: 0.0, pitch: 0.0, iniciado: false }
    }

    /// Actualiza con una lectura y el intervalo `dt` en segundos.
    pub fn actualizar(&mut self, imu: &Imu, dt: f32) -> (f32, f32) {
        let (r_a, p_a) = angulos_acel(imu.acel);
        if !self.iniciado {
            // Primera muestra: arrancar desde el acelerómetro evita un
            // transitorio de ~τ en el que el dron "cree" estar nivelado.
            self.roll = r_a;
            self.pitch = p_a;
            self.iniciado = true;
        } else {
            let a = self.alfa;
            self.roll = a * (self.roll + imu.giro[0] * dt) + (1.0 - a) * r_a;
            self.pitch = a * (self.pitch + imu.giro[1] * dt) + (1.0 - a) * p_a;
        }
        (self.roll, self.pitch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angulos_desde_gravedad() {
        let (r, p) = angulos_acel([0.0, 0.5, 0.866]);
        assert!((r - 30.0).abs() < 0.1 && p.abs() < 0.1);
        let (r, p) = angulos_acel([-0.5, 0.0, 0.866]);
        assert!(r.abs() < 0.1 && (p - 30.0).abs() < 0.1);
    }

    #[test]
    fn corrige_deriva_del_giroscopio() {
        // El dron está nivelado pero el giro tiene un bias de 2 °/s.
        // Integrando solo el giro, tras 10 s el error sería 20°. Con el filtro
        // queda acotado en ≈ bias·τ ≈ 0,1°.
        let mut f = Complementario::new(0.98);
        let imu = Imu { acel: [0.0, 0.0, 1.0], giro: [2.0, 0.0, 0.0] };
        for _ in 0..10_000 {
            f.actualizar(&imu, 0.001);
        }
        assert!(f.roll.abs() < 0.2, "roll = {}", f.roll);
    }
}
