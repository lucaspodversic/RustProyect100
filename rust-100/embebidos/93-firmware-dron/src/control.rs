//! Controlador de vuelo: une actitud, PIDs, mezclador y seguridad.

use crate::actitud::{Complementario, Imu};
use crate::mezclador::mezclar;
use crate::motores::RALENTI;
use crate::pid::Pid;
use crate::seguridad::{Estado, Limites, Seguridad};

/// Entradas del radiocontrol, normalizadas.
#[derive(Debug, Clone, Copy, Default)]
pub struct Radio {
    /// Acelerador 0..1.
    pub acelerador: f32,
    /// Palancas −1..1.
    pub roll: f32,
    /// Pitch −1..1.
    pub pitch: f32,
    /// Yaw −1..1.
    pub yaw: f32,
    /// ¿Hay enlace?
    pub enlace: bool,
}

/// Ganancias del controlador.
#[derive(Debug, Clone, Copy)]
pub struct Ganancias {
    /// Ganancia P del lazo externo (°/s por grado de error).
    pub angulo_p: f32,
    /// PID de velocidad roll/pitch: (kp, ki, kd).
    pub velocidad: (f32, f32, f32),
    /// PID de velocidad yaw.
    pub yaw: (f32, f32, f32),
    /// Ángulo máximo con la palanca a fondo (modo ángulo), en grados.
    pub angulo_max: f32,
    /// Velocidad de yaw máxima (°/s).
    pub yaw_max: f32,
}

impl Default for Ganancias {
    fn default() -> Self {
        // Ganancias ajustadas para el modelo de `simulacion` (dron de ~250 mm).
        Self { angulo_p: 5.0, velocidad: (0.0025, 0.004, 0.00003), yaw: (0.004, 0.002, 0.0), angulo_max: 30.0, yaw_max: 180.0 }
    }
}

/// Telemetría de un ciclo.
#[derive(Debug, Clone, Copy)]
pub struct Telemetria {
    /// Estado de seguridad.
    pub estado: Estado,
    /// Roll estimado.
    pub roll: f32,
    /// Pitch estimado.
    pub pitch: f32,
    /// Empuje de los motores.
    pub motores: [f32; 4],
}

/// Controlador de vuelo.
pub struct Controlador {
    /// Estimador de actitud.
    pub actitud: Complementario,
    /// Seguridad.
    pub seguridad: Seguridad,
    g: Ganancias,
    pid_roll: Pid,
    pid_pitch: Pid,
    pid_yaw: Pid,
}

impl Controlador {
    /// Crea el controlador.
    pub fn new(g: Ganancias, limites: Limites) -> Self {
        let pid = |(kp, ki, kd): (f32, f32, f32)| Pid::new(kp, ki, kd, 0.2, 0.5);
        Self {
            actitud: Complementario::new(0.98),
            seguridad: Seguridad::new(limites),
            g,
            pid_roll: pid(g.velocidad),
            pid_pitch: pid(g.velocidad),
            pid_yaw: pid(g.yaw),
        }
    }

    /// Intenta armar con la actitud actual.
    pub fn armar(&mut self, r: &Radio) -> Result<(), crate::seguridad::RechazoArmado> {
        self.seguridad.armar(r.acelerador, self.actitud.roll, self.actitud.pitch, r.enlace)
    }

    /// Un ciclo del lazo de control (normalmente 1 kHz).
    pub fn ciclo(&mut self, imu: &Imu, radio: &Radio, dt: f32) -> Telemetria {
        let (roll, pitch) = self.actitud.actualizar(imu, dt);
        let estado = self.seguridad.actualizar(dt, radio.enlace, radio.acelerador, roll, pitch);

        // En failsafe se ignoran las palancas: nivelado y descenso suave.
        let (acel, sp_roll, sp_pitch, sp_yaw) = match estado {
            Estado::Desarmado => {
                self.pid_roll.reiniciar();
                self.pid_pitch.reiniciar();
                self.pid_yaw.reiniciar();
                return Telemetria { estado, roll, pitch, motores: [0.0; 4] };
            }
            Estado::Armado => (
                radio.acelerador,
                radio.roll * self.g.angulo_max,
                radio.pitch * self.g.angulo_max,
                radio.yaw * self.g.yaw_max,
            ),
            Estado::Failsafe { .. } => (self.seguridad.limites.acelerador_failsafe, 0.0, 0.0, 0.0),
        };

        // Con el acelerador casi a cero (en el suelo) no se integra: si no,
        // la integral acumularía el error de un suelo inclinado y el dron
        // saltaría de lado al despegar.
        if acel < 0.1 {
            self.pid_roll.reiniciar();
            self.pid_pitch.reiniciar();
            self.pid_yaw.reiniciar();
        }

        // Lazo externo (ángulo → velocidad deseada) e interno (velocidad → comando).
        let sp_vr = (sp_roll - roll) * self.g.angulo_p;
        let sp_vp = (sp_pitch - pitch) * self.g.angulo_p;
        let c_roll = self.pid_roll.actualizar(sp_vr, imu.giro[0], dt);
        let c_pitch = self.pid_pitch.actualizar(sp_vp, imu.giro[1], dt);
        let c_yaw = self.pid_yaw.actualizar(sp_yaw, imu.giro[2], dt);

        let acel = RALENTI + acel * (1.0 - RALENTI);
        let motores = mezclar(acel, c_roll, c_pitch, c_yaw);
        Telemetria { estado, roll, pitch, motores }
    }
}
