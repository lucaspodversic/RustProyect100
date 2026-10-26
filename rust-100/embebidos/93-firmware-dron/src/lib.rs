//! # Firmware básico de dron (cuadricóptero en X)
//!
//! Lazo de control típico de un controlador de vuelo, ejecutado a 1 kHz:
//!
//! ```text
//!  IMU ─► [actitud: filtro complementario] ─► roll/pitch estimados
//!                                                   │
//!  radio ─► [modo ángulo: setpoint] ─► PID ángulo ─► setpoint de velocidad
//!                                                   │
//!  giroscopio ───────────────────────► PID velocidad ─► comandos roll/pitch/yaw
//!                                                   │
//!                     acelerador ─► [mezclador X] ─► 4 motores (PWM a los ESC)
//! ```
//!
//! Además hay una **máquina de estados de seguridad** ([`seguridad`]). Solo
//! permite armar con el acelerador al mínimo y el dron nivelado. Si se pierde
//! la radio activa el *failsafe* (desciende y luego desarma), y si el dron
//! vuelca, desarma.
//!
//! Como no hay hardware, [`simulacion`] modela la dinámica rotacional del
//! cuadricóptero (inercia, empuje con retardo del motor, ruido del IMU). Los
//! tests comprueban que el lazo estabiliza el dron tras una perturbación.
//!
//! **Convenciones**: ángulos en grados y velocidades en °/s. Roll positivo
//! es inclinarse a la derecha y pitch positivo es subir el morro. Yaw
//! positivo es girar en sentido horario visto desde arriba.
#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod actitud;
pub mod control;
pub mod mezclador;
pub mod motores;
pub mod pid;
pub mod seguridad;
pub mod simulacion;
