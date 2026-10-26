//! Estado y configuración de cada zona de riego.

use crate::sensor::Calibracion;

/// Configuración de una zona.
#[derive(Debug, Clone, Copy)]
pub struct ConfigZona {
    /// Humedad (%) por debajo de la cual se pide riego.
    pub umbral_bajo: u8,
    /// Humedad (%) a la que se corta.
    pub umbral_alto: u8,
    /// Tiempo máximo de un ciclo de riego.
    pub max_riego_ms: u32,
    /// Pausa de remojo tras regar.
    pub remojo_ms: u32,
    /// Caudal del gotero/aspersor en mililitros por segundo (para estadísticas).
    pub caudal_ml_s: u32,
    /// Calibración del sensor.
    pub calibracion: Calibracion,
}

impl Default for ConfigZona {
    fn default() -> Self {
        Self {
            umbral_bajo: 30,
            umbral_alto: 45,
            max_riego_ms: 10 * 60_000,
            remojo_ms: 20 * 60_000,
            caudal_ml_s: 50,
            calibracion: Calibracion::default(),
        }
    }
}

/// Estado de la zona.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoZona {
    /// Humedad suficiente, nada que hacer.
    Ok,
    /// Pide riego; espera turno (otra zona puede estar regando).
    Pendiente,
    /// Regando: ms transcurridos.
    Regando(u32),
    /// Remojo: ms restantes.
    Remojo(u32),
    /// Sensor en falla: zona deshabilitada hasta que vuelva a leer bien.
    FallaSensor,
}

/// Datos de la zona en tiempo de ejecución.
#[derive(Debug, Clone, Copy)]
pub struct Zona {
    /// Configuración.
    pub cfg: ConfigZona,
    /// Estado actual.
    pub estado: EstadoZona,
    /// Última humedad válida leída.
    pub humedad: Option<u8>,
    /// Agua usada hoy (mL).
    pub ml_hoy: u32,
    /// Ciclos de riego hoy.
    pub ciclos_hoy: u16,
}

impl Zona {
    /// Nueva zona con la configuración dada.
    pub const fn new(cfg: ConfigZona) -> Self {
        Self { cfg, estado: EstadoZona::Ok, humedad: None, ml_hoy: 0, ciclos_hoy: 0 }
    }

    /// Reinicia los contadores diarios (llamar a medianoche).
    pub fn nuevo_dia(&mut self) {
        self.ml_hoy = 0;
        self.ciclos_hoy = 0;
    }
}
