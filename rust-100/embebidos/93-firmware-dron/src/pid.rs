//! Controlador PID con las mejoras habituales en drones:
//!
//! - **Derivada sobre la medición** (no sobre el error): así un salto del
//!   setpoint no genera un pico en el término D (*derivative kick*).
//! - **Anti‑windup**: la integral queda limitada a `±limite_i`. Además no se
//!   integra mientras el dron está en el suelo (ver `reiniciar`).
//! - **Salida saturada** a `±limite_salida`.

/// Parámetros y estado de un PID.
#[derive(Debug, Clone, Copy)]
pub struct Pid {
    /// Ganancia proporcional.
    pub kp: f32,
    /// Ganancia integral.
    pub ki: f32,
    /// Ganancia derivativa.
    pub kd: f32,
    /// Límite del término integral (en unidades de salida).
    pub limite_i: f32,
    /// Límite de la salida.
    pub limite_salida: f32,
    integral: f32,
    medicion_previa: Option<f32>,
}

impl Pid {
    /// Crea un PID.
    pub const fn new(kp: f32, ki: f32, kd: f32, limite_i: f32, limite_salida: f32) -> Self {
        Self { kp, ki, kd, limite_i, limite_salida, integral: 0.0, medicion_previa: None }
    }

    /// Borra la integral y la memoria de la derivada.
    pub fn reiniciar(&mut self) {
        self.integral = 0.0;
        self.medicion_previa = None;
    }

    /// Término integral actual (para tests y telemetría).
    pub fn integral(&self) -> f32 {
        self.integral
    }

    /// Calcula la salida.
    pub fn actualizar(&mut self, setpoint: f32, medicion: f32, dt: f32) -> f32 {
        let error = setpoint - medicion;
        self.integral = (self.integral + self.ki * error * dt).clamp(-self.limite_i, self.limite_i);
        let derivada = match self.medicion_previa {
            Some(prev) if dt > 0.0 => -(medicion - prev) / dt,
            _ => 0.0,
        };
        self.medicion_previa = Some(medicion);
        (self.kp * error + self.integral + self.kd * derivada).clamp(-self.limite_salida, self.limite_salida)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proporcional_y_saturacion() {
        let mut p = Pid::new(2.0, 0.0, 0.0, 0.0, 5.0);
        assert_eq!(p.actualizar(1.0, 0.0, 0.01), 2.0);
        assert_eq!(p.actualizar(10.0, 0.0, 0.01), 5.0);
    }

    #[test]
    fn anti_windup() {
        let mut p = Pid::new(0.0, 10.0, 0.0, 0.3, 1.0);
        for _ in 0..1000 {
            p.actualizar(100.0, 0.0, 0.01);
        }
        assert_eq!(p.integral(), 0.3);
        p.reiniciar();
        assert_eq!(p.integral(), 0.0);
    }

    #[test]
    fn sin_patada_derivativa() {
        let mut p = Pid::new(0.0, 0.0, 1.0, 0.0, 100.0);
        p.actualizar(0.0, 5.0, 0.01);
        // El setpoint salta de 0 a 50 pero la medición no cambia → D = 0.
        assert_eq!(p.actualizar(50.0, 5.0, 0.01), 0.0);
        // La medición sube 1 en 10 ms → D = −100 (frena el movimiento).
        assert_eq!(p.actualizar(50.0, 6.0, 0.01), -100.0);
    }
}
