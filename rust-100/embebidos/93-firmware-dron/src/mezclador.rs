//! Mezclador para cuadricóptero en configuración **X**.
//!
//! ```text
//!        frente
//!   M3(DI) ▲ M1(DD)       DI = delantero izquierdo (gira CW)
//!       ╲  │  ╱           DD = delantero derecho   (gira CCW)
//!        ╲ │ ╱
//!         ███
//!        ╱   ╲
//!       ╱     ╲
//!   M2(TI)   M0(TD)       TI = trasero izquierdo (gira CCW)
//!                         TD = trasero derecho   (gira CW)
//! ```
//!
//! Para cada motor: `salida = acelerador + kr·roll + kp·pitch + ky·yaw`
//!
//! | motor | roll | pitch | yaw |
//! |---|---|---|---|
//! | M0 TD | −1 | −1 | −1 |
//! | M1 DD | −1 | +1 | +1 |
//! | M2 TI | +1 | −1 | +1 |
//! | M3 DI | +1 | +1 | −1 |
//!
//! - Roll a la derecha: los motores izquierdos empujan más.
//! - Pitch hacia arriba (morro arriba): los delanteros empujan más.
//! - Yaw horario: se aceleran los que giran CCW, porque su par de reacción es CW.
//!
//! **Desaturación**: si algún motor pide más de 1 (o menos de 0), se
//! desplaza todo el conjunto para conservar la *diferencia* entre motores,
//! que es lo que mantiene la actitud, a costa del empuje total. Sin esto,
//! a plena potencia el dron perdería el control.

/// Índices de los motores.
pub mod motor {
    /// Trasero derecho.
    pub const TD: usize = 0;
    /// Delantero derecho.
    pub const DD: usize = 1;
    /// Trasero izquierdo.
    pub const TI: usize = 2;
    /// Delantero izquierdo.
    pub const DI: usize = 3;
}

/// Tabla de mezcla `[roll, pitch, yaw]` por motor.
pub const TABLA_X: [[f32; 3]; 4] = [
    [-1.0, -1.0, -1.0],
    [-1.0, 1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, -1.0],
];

/// Mezcla y desatura. Todas las entradas están normalizadas: acelerador
/// 0..1 y comandos aproximadamente −1..1. La salida queda en `[0, 1]` por motor.
pub fn mezclar(acelerador: f32, roll: f32, pitch: f32, yaw: f32) -> [f32; 4] {
    let mut m = [0.0f32; 4];
    for (i, f) in TABLA_X.iter().enumerate() {
        m[i] = acelerador + f[0] * roll + f[1] * pitch + f[2] * yaw;
    }
    let max = m.iter().cloned().fold(f32::MIN, f32::max);
    let min = m.iter().cloned().fold(f32::MAX, f32::min);
    let rango = max - min;
    if rango > 1.0 {
        // Ni desplazando cabe: se escala la parte diferencial.
        let medio = (max + min) / 2.0;
        for v in m.iter_mut() {
            *v = (*v - medio) / rango + 0.5;
        }
    } else if max > 1.0 {
        let d = max - 1.0;
        m.iter_mut().for_each(|v| *v -= d);
    } else if min < 0.0 {
        m.iter_mut().for_each(|v| *v -= min);
    }
    m.map(|v| v.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signos_de_la_mezcla() {
        let m = mezclar(0.5, 0.1, 0.0, 0.0);
        assert!(m[motor::TI] > m[motor::TD] && m[motor::DI] > m[motor::DD]);
        let m = mezclar(0.5, 0.0, 0.1, 0.0);
        assert!(m[motor::DD] > m[motor::TD] && m[motor::DI] > m[motor::TI]);
        let m = mezclar(0.5, 0.0, 0.0, 0.1);
        assert!(m[motor::DD] > m[motor::DI] && m[motor::TI] > m[motor::TD]);
    }

    #[test]
    fn desaturacion_conserva_diferencias() {
        // Acelerador a fondo + roll: sin desaturar, los izquierdos se cortarían en 1.
        let m = mezclar(0.95, 0.2, 0.0, 0.0);
        let dif = m[motor::TI] - m[motor::TD];
        assert!((dif - 0.4).abs() < 1e-6, "{m:?}");
        assert!(m.iter().all(|&v| (0.0..=1.0).contains(&v)));
        // Al ralentí con un comando negativo: se sube el conjunto.
        let m = mezclar(0.0, -0.1, 0.0, 0.0);
        assert!((m[motor::TD] - m[motor::TI] - 0.2).abs() < 1e-6);
        // Rango imposible (> 1): se escala manteniendo el orden.
        let m = mezclar(0.5, 1.0, 0.0, 0.0);
        assert!(m[motor::TI] == 1.0 && m[motor::TD] == 0.0);
    }
}
