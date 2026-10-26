//! Orquestador de la estación.
//!
//! Ciclo de vida (llamado periódicamente por el `main` del firmware, p. ej.
//! cada 10 s desde un timer o una tarea de Embassy):
//!
//! ```text
//!  tick() ──► medir SHT3x ──► acumular en la ventana
//!                 │
//!                 └─ cada `muestras_por_envio` ticks ──► armar JSON ──► enviar
//!                                                       (con reintentos)
//! ```
//!
//! Si el envío falla la ventana **no** se reinicia: la próxima publicación
//! incluirá también estas muestras (no se pierden datos por un corte de Wi‑Fi).

use core::fmt::Write;
use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;

use crate::estadisticas::{punto_de_rocio_aprox, Ventana};
use crate::json::{BufferFijo, Centesimas};
use crate::red::{enviar_con_reintentos, ErrorRed, PoliticaReintentos, Transporte};
use crate::sht3x::{self, Lectura, Sht3x};

/// Resultado de un `tick`.
#[derive(Debug, PartialEq, Eq)]
pub enum Evento {
    /// Se midió y acumuló (todavía no toca publicar).
    Medido(Lectura),
    /// Se publicó el resumen (cantidad de intentos usados).
    Publicado {
        /// Lectura del tick actual.
        lectura: Lectura,
        /// Intentos de red que hicieron falta.
        intentos: u8,
    },
    /// Falló la publicación; los datos quedan pendientes.
    PublicacionFallida(ErrorRed),
    /// Falló el sensor (el tick se descarta pero la estación sigue viva).
    ErrorSensor,
}

/// Configuración de la estación.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Identificador que viaja en el JSON.
    pub id_estacion: u16,
    /// Cada cuántas mediciones se publica.
    pub muestras_por_envio: u32,
    /// Política de reintentos de red.
    pub reintentos: PoliticaReintentos,
}

/// La estación completa.
pub struct Estacion<I2C, T> {
    sensor: Sht3x<I2C>,
    red: T,
    cfg: Config,
    ventana: Ventana,
    secuencia: u32,
    /// Último JSON armado (útil para depurar por UART).
    pub ultimo_json: BufferFijo<192>,
}

impl<I2C: I2c, T: Transporte> Estacion<I2C, T> {
    /// Crea la estación.
    pub fn new(sensor: Sht3x<I2C>, red: T, cfg: Config) -> Self {
        Self {
            sensor,
            red,
            cfg,
            ventana: Ventana::default(),
            secuencia: 0,
            ultimo_json: BufferFijo::new(),
        }
    }

    /// Acceso al transporte (para inspección en tests).
    pub fn red(&self) -> &T {
        &self.red
    }

    /// Devuelve sensor y red (desarma la estación).
    pub fn desarmar(self) -> (Sht3x<I2C>, T) {
        (self.sensor, self.red)
    }

    /// Un paso del ciclo principal.
    pub fn tick<D: DelayNs>(&mut self, delay: &mut D) -> Evento {
        let lectura = match self.sensor.medir(delay) {
            Ok(l) => l,
            Err(_) => return Evento::ErrorSensor,
        };
        self.ventana.agregar(&lectura);
        if self.ventana.temperatura.cantidad() < self.cfg.muestras_por_envio {
            return Evento::Medido(lectura);
        }
        // Toca publicar: armamos el JSON en el buffer fijo.
        if self.armar_json(&lectura).is_err() {
            // No debería pasar con 192 bytes; si pasa, reiniciamos para no
            // quedar atascados.
            self.ventana = Ventana::default();
            return Evento::ErrorSensor;
        }
        match enviar_con_reintentos(
            &mut self.red,
            delay,
            self.ultimo_json.as_bytes(),
            self.cfg.reintentos,
        ) {
            Ok(intentos) => {
                self.ventana = Ventana::default();
                self.secuencia = self.secuencia.wrapping_add(1);
                Evento::Publicado { lectura, intentos }
            }
            Err(e) => Evento::PublicacionFallida(e),
        }
    }

    /// Arma el JSON del resumen, por ejemplo:
    /// `{"id":7,"seq":0,"n":3,"t":{"min":..,"max":..,"avg":..},"h":{..},"rocio":..}`
    fn armar_json(&mut self, actual: &Lectura) -> core::fmt::Result {
        let t = self.ventana.temperatura.resumen().ok_or(core::fmt::Error)?;
        let h = self.ventana.humedad.resumen().ok_or(core::fmt::Error)?;
        let rocio = punto_de_rocio_aprox(actual.temp_centi_c, actual.humedad_centi_pct);
        let b = &mut self.ultimo_json;
        b.limpiar();
        write!(
            b,
            "{{\"id\":{},\"seq\":{},\"n\":{},\"t\":{{\"min\":{},\"max\":{},\"avg\":{}}},\
             \"h\":{{\"min\":{},\"max\":{},\"avg\":{}}},\"rocio\":{}}}",
            self.cfg.id_estacion,
            self.secuencia,
            self.ventana.temperatura.cantidad(),
            Centesimas(t.min),
            Centesimas(t.max),
            Centesimas(t.promedio),
            Centesimas(h.min),
            Centesimas(h.max),
            Centesimas(h.promedio),
            Centesimas(rocio),
        )
    }
}

/// Atajo para la dirección por defecto del sensor.
pub fn sensor_por_defecto<I2C: I2c>(i2c: I2C) -> Sht3x<I2C> {
    Sht3x::new(i2c, sht3x::DIRECCION_POR_DEFECTO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::red::falso::WifiFalso;
    use crate::sht3x::{crc8, CMD_MEDIR_ALTA};
    use embedded_hal_mock::eh1::delay::NoopDelay;
    use embedded_hal_mock::eh1::i2c::{Mock, Transaction};

    fn medicion(raw_t: u16, raw_h: u16) -> [Transaction; 2] {
        let t = raw_t.to_be_bytes();
        let h = raw_h.to_be_bytes();
        [
            Transaction::write(0x44, CMD_MEDIR_ALTA.to_vec()),
            Transaction::read(0x44, vec![t[0], t[1], crc8(&t), h[0], h[1], crc8(&h)]),
        ]
    }

    fn cfg() -> Config {
        Config { id_estacion: 7, muestras_por_envio: 2, reintentos: Default::default() }
    }

    #[test]
    fn publica_cada_n_muestras() {
        let mut tx = Vec::new();
        tx.extend(medicion(0x6666, 0x8000)); // 25.00 °C, 50.00 %
        tx.extend(medicion(0x6000, 0x9999)); // ~20.63 °C, ~60.00 %
        let mut est = Estacion::new(sensor_por_defecto(Mock::new(&tx)), WifiFalso::default(), cfg());
        let mut d = NoopDelay::new();
        assert!(matches!(est.tick(&mut d), Evento::Medido(_)));
        assert!(matches!(est.tick(&mut d), Evento::Publicado { intentos: 1, .. }));
        let json = std::str::from_utf8(&est.red().enviados[0]).unwrap().to_string();
        assert!(json.starts_with("{\"id\":7,\"seq\":0,\"n\":2,"), "{json}");
        assert!(json.contains("\"max\":25.00"), "{json}");
        assert!(json.contains("\"max\":60.00"), "{json}");
        let (s, _) = est.desarmar();
        s.liberar().done();
    }

    #[test]
    fn si_falla_la_red_no_pierde_muestras() {
        let mut tx = Vec::new();
        for _ in 0..3 {
            tx.extend(medicion(0x6666, 0x8000));
        }
        let wifi = WifiFalso { fallos_restantes: 100, ..Default::default() };
        let mut c = cfg();
        c.reintentos.intentos = 1;
        let mut est = Estacion::new(sensor_por_defecto(Mock::new(&tx)), wifi, c);
        let mut d = NoopDelay::new();
        est.tick(&mut d);
        assert!(matches!(est.tick(&mut d), Evento::PublicacionFallida(_)));
        // Se cae el error: el próximo tick vuelve a intentar con n=3.
        assert!(matches!(est.tick(&mut d), Evento::PublicacionFallida(_)));
        assert!(est.ultimo_json.as_str().contains("\"n\":3"));
        let (s, _) = est.desarmar();
        s.liberar().done();
    }
}
