//! Máquina de estados de seguridad (armado / failsafe).
//!
//! ```text
//!            armar (acel. bajo, nivelado, radio OK)
//!  Desarmado ───────────────────────────────────────► Armado
//!      ▲  ▲                                            │  │
//!      │  └────────── desarmar / vuelco ───────────────┘  │ radio perdida > 0,5 s
//!      │                                                  ▼
//!      └──── 5 s descendiendo o radio vuelve con acel. bajo ── Failsafe
//! ```

/// Estado del vehículo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Estado {
    /// Motores parados.
    Desarmado,
    /// Volando (o listo en el suelo) bajo control del piloto.
    Armado,
    /// Sin radio: se nivela y desciende; `t` = segundos en failsafe.
    Failsafe {
        /// Tiempo transcurrido en failsafe.
        t: f32,
    },
}

/// Motivo de rechazo al armar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RechazoArmado {
    /// El acelerador no está al mínimo.
    AceleradorAlto,
    /// El dron no está nivelado (> 25°).
    NoNivelado,
    /// Sin enlace de radio.
    SinRadio,
}

/// Parámetros de seguridad.
#[derive(Debug, Clone, Copy)]
pub struct Limites {
    /// Inclinación máxima para permitir armar (°).
    pub max_inclinacion_armado: f32,
    /// Inclinación a partir de la cual se asume vuelco y se desarma (°).
    pub angulo_vuelco: f32,
    /// Tiempo sin radio antes de activar failsafe (s).
    pub timeout_radio: f32,
    /// Duración del descenso de failsafe antes de desarmar (s).
    pub duracion_failsafe: f32,
    /// Acelerador durante el descenso de failsafe (algo menos que el hover).
    pub acelerador_failsafe: f32,
}

impl Default for Limites {
    fn default() -> Self {
        Self {
            max_inclinacion_armado: 25.0,
            angulo_vuelco: 70.0,
            timeout_radio: 0.5,
            duracion_failsafe: 5.0,
            acelerador_failsafe: 0.4,
        }
    }
}

/// Supervisor de seguridad.
#[derive(Debug, Clone, Copy)]
pub struct Seguridad {
    /// Estado actual.
    pub estado: Estado,
    /// Parámetros.
    pub limites: Limites,
    sin_radio: f32,
}

impl Seguridad {
    /// Arranca desarmado.
    pub fn new(limites: Limites) -> Self {
        Self { estado: Estado::Desarmado, limites, sin_radio: 0.0 }
    }

    /// Petición de armado.
    pub fn armar(&mut self, acelerador: f32, roll: f32, pitch: f32, radio_ok: bool) -> Result<(), RechazoArmado> {
        if !radio_ok {
            return Err(RechazoArmado::SinRadio);
        }
        if acelerador > 0.05 {
            return Err(RechazoArmado::AceleradorAlto);
        }
        let m = self.limites.max_inclinacion_armado;
        if roll.abs() > m || pitch.abs() > m {
            return Err(RechazoArmado::NoNivelado);
        }
        self.estado = Estado::Armado;
        Ok(())
    }

    /// Desarma (siempre permitido: es el "botón de pánico").
    pub fn desarmar(&mut self) {
        self.estado = Estado::Desarmado;
    }

    /// Actualiza el supervisor en cada ciclo.
    pub fn actualizar(&mut self, dt: f32, radio_ok: bool, acelerador: f32, roll: f32, pitch: f32) -> Estado {
        self.sin_radio = if radio_ok { 0.0 } else { self.sin_radio + dt };
        let vuelco = roll.abs() > self.limites.angulo_vuelco || pitch.abs() > self.limites.angulo_vuelco;
        self.estado = match self.estado {
            Estado::Desarmado => Estado::Desarmado,
            _ if vuelco => Estado::Desarmado,
            Estado::Armado if self.sin_radio > self.limites.timeout_radio => Estado::Failsafe { t: 0.0 },
            Estado::Armado => Estado::Armado,
            // El piloto recupera el control solo si la radio vuelve y baja
            // el acelerador (evita un salto brusco de potencia).
            Estado::Failsafe { .. } if radio_ok && acelerador < 0.05 => Estado::Armado,
            Estado::Failsafe { t } if t + dt >= self.limites.duracion_failsafe => Estado::Desarmado,
            Estado::Failsafe { t } => Estado::Failsafe { t: t + dt },
        };
        self.estado
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condiciones_de_armado() {
        let mut s = Seguridad::new(Limites::default());
        assert_eq!(s.armar(0.3, 0.0, 0.0, true), Err(RechazoArmado::AceleradorAlto));
        assert_eq!(s.armar(0.0, 30.0, 0.0, true), Err(RechazoArmado::NoNivelado));
        assert_eq!(s.armar(0.0, 0.0, 0.0, false), Err(RechazoArmado::SinRadio));
        assert_eq!(s.armar(0.0, 3.0, -2.0, true), Ok(()));
        assert_eq!(s.estado, Estado::Armado);
    }

    #[test]
    fn failsafe_y_desarmado() {
        let mut s = Seguridad::new(Limites::default());
        s.armar(0.0, 0.0, 0.0, true).unwrap();
        for _ in 0..400 {
            assert_eq!(s.actualizar(0.001, false, 0.5, 0.0, 0.0), Estado::Armado);
        }
        for _ in 0..200 {
            s.actualizar(0.001, false, 0.5, 0.0, 0.0);
        }
        assert!(matches!(s.estado, Estado::Failsafe { .. }));
        for _ in 0..5000 {
            s.actualizar(0.001, false, 0.5, 0.0, 0.0);
        }
        assert_eq!(s.estado, Estado::Desarmado);
    }

    #[test]
    fn vuelco_desarma() {
        let mut s = Seguridad::new(Limites::default());
        s.armar(0.0, 0.0, 0.0, true).unwrap();
        assert_eq!(s.actualizar(0.001, true, 0.5, 80.0, 0.0), Estado::Desarmado);
    }
}
