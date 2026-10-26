//! Capa de red abstracta ("Wi‑Fi").
//!
//! En el ESP32 real el transporte sería, por ejemplo, un cliente HTTP o MQTT
//! sobre `esp-wifi` + `embassy-net`. Para que la lógica sea testeable lo
//! modelamos con un trait mínimo: *conectar* y *enviar un payload*.
//!
//! La política de reintentos usa **backoff exponencial con tope**: tras cada
//! fallo duplicamos la espera (500 ms, 1 s, 2 s, …) hasta `max_espera_ms`.
//! Esto evita saturar un router que está caído y ahorra batería.

use embedded_hal::delay::DelayNs;

/// Errores genéricos del transporte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorRed {
    /// No hay asociación con el punto de acceso.
    SinConexion,
    /// El servidor no respondió a tiempo.
    Timeout,
    /// El servidor respondió con error (p. ej. HTTP 5xx).
    Rechazado,
}

/// Trait que debe implementar cualquier transporte de red.
pub trait Transporte {
    /// Intenta (re)conectar al AP. Devuelve `Ok` si quedó conectado.
    fn conectar(&mut self) -> Result<(), ErrorRed>;
    /// ¿Está conectado ahora?
    fn conectado(&self) -> bool;
    /// Envía un payload (JSON) al servidor.
    fn enviar(&mut self, payload: &[u8]) -> Result<(), ErrorRed>;
}

/// Parámetros de la política de reintentos.
#[derive(Debug, Clone, Copy)]
pub struct PoliticaReintentos {
    /// Cantidad máxima de intentos (incluye el primero).
    pub intentos: u8,
    /// Espera inicial entre intentos.
    pub espera_inicial_ms: u32,
    /// Tope de la espera.
    pub max_espera_ms: u32,
}

impl Default for PoliticaReintentos {
    fn default() -> Self {
        Self { intentos: 4, espera_inicial_ms: 500, max_espera_ms: 4000 }
    }
}

/// Envía con reintentos. Si el transporte se desconectó, reconecta antes de
/// reintentar. Devuelve la cantidad de intentos usados en caso de éxito.
pub fn enviar_con_reintentos<T: Transporte, D: DelayNs>(
    t: &mut T,
    delay: &mut D,
    payload: &[u8],
    pol: PoliticaReintentos,
) -> Result<u8, ErrorRed> {
    let mut espera = pol.espera_inicial_ms;
    let mut ultimo_error = ErrorRed::SinConexion;
    for intento in 1..=pol.intentos {
        // Si no hay conexión, primero intentamos asociarnos.
        let resultado = if !t.conectado() {
            t.conectar().and_then(|_| t.enviar(payload))
        } else {
            t.enviar(payload)
        };
        match resultado {
            Ok(()) => return Ok(intento),
            Err(e) => ultimo_error = e,
        }
        // No esperamos después del último intento.
        if intento < pol.intentos {
            delay.delay_ms(espera);
            espera = (espera.saturating_mul(2)).min(pol.max_espera_ms);
        }
    }
    Err(ultimo_error)
}

#[cfg(test)]
pub(crate) mod falso {
    //! Wi‑Fi falso para tests: registra lo enviado y puede fallar N veces.
    use super::*;
    use std::vec::Vec;

    /// Transporte simulado.
    #[derive(Default)]
    pub struct WifiFalso {
        pub conectado: bool,
        pub fallos_restantes: u32,
        pub enviados: Vec<Vec<u8>>,
        pub conexiones: u32,
    }

    impl Transporte for WifiFalso {
        fn conectar(&mut self) -> Result<(), ErrorRed> {
            self.conexiones += 1;
            self.conectado = true;
            Ok(())
        }
        fn conectado(&self) -> bool {
            self.conectado
        }
        fn enviar(&mut self, payload: &[u8]) -> Result<(), ErrorRed> {
            if self.fallos_restantes > 0 {
                self.fallos_restantes -= 1;
                self.conectado = false; // simulamos que se cayó la conexión
                return Err(ErrorRed::Timeout);
            }
            self.enviados.push(payload.to_vec());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::falso::WifiFalso;
    use super::*;
    use embedded_hal_mock::eh1::delay::{CheckedDelay, Transaction};

    #[test]
    fn exito_al_primer_intento_conecta_si_hace_falta() {
        let mut w = WifiFalso::default();
        let mut d = CheckedDelay::new(&[]);
        let n = enviar_con_reintentos(&mut w, &mut d, b"x", Default::default()).unwrap();
        assert_eq!(n, 1);
        assert_eq!(w.conexiones, 1);
        d.done();
    }

    #[test]
    fn backoff_exponencial_con_tope() {
        let mut w = WifiFalso { conectado: true, fallos_restantes: 3, ..Default::default() };
        // Esperas esperadas: 500, 1000, 2000 (y el 4º intento sale bien).
        let mut d = CheckedDelay::new(&[
            Transaction::delay_ms(500),
            Transaction::delay_ms(1000),
            Transaction::delay_ms(2000),
        ]);
        let n = enviar_con_reintentos(&mut w, &mut d, b"hola", Default::default()).unwrap();
        assert_eq!(n, 4);
        assert_eq!(w.enviados, vec![b"hola".to_vec()]);
        d.done();
    }

    #[test]
    fn agota_intentos() {
        let mut w = WifiFalso { conectado: true, fallos_restantes: 10, ..Default::default() };
        let pol = PoliticaReintentos { intentos: 2, espera_inicial_ms: 10, max_espera_ms: 10 };
        let mut d = CheckedDelay::new(&[Transaction::delay_ms(10)]);
        assert_eq!(
            enviar_con_reintentos(&mut w, &mut d, b"x", pol),
            Err(ErrorRed::Timeout)
        );
        d.done();
    }
}
