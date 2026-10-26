//! Abstracción del servicio externo de clima.
//!
//! El trait [`Proveedor`] permite inyectar:
//! - [`OpenMeteo`]: cliente real (API pública sin clave, `api.open-meteo.com`),
//!   con `base_url` configurable → en los tests apunta a un servidor local falso;
//! - cualquier *mock* en memoria (ver tests).

use serde::{Deserialize, Serialize};
use std::future::Future;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Clima {
    pub temperatura_c: f64,
    pub viento_kmh: f64,
    pub codigo: i64,
    pub descripcion: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorProveedor {
    /// Timeout o error de red/HTTP 5xx: transitorio.
    NoDisponible(String),
    /// Respuesta con formato inesperado.
    Formato(String),
}

/// `Send + Sync + 'static` para poder compartirlo entre tareas de Axum.
pub trait Proveedor: Send + Sync + 'static {
    fn consultar(&self, lat: f64, lon: f64) -> impl Future<Output = Result<Clima, ErrorProveedor>> + Send;
}

/// Traduce el código WMO de Open-Meteo a texto.
pub fn describir_wmo(c: i64) -> &'static str {
    match c {
        0 => "despejado",
        1..=3 => "parcialmente nublado",
        45 | 48 => "niebla",
        51..=57 => "llovizna",
        61..=67 | 80..=82 => "lluvia",
        71..=77 | 85 | 86 => "nieve",
        95..=99 => "tormenta",
        _ => "desconocido",
    }
}

pub struct OpenMeteo {
    http: reqwest::Client,
    base_url: String,
}

impl OpenMeteo {
    pub fn new(base_url: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5)) // nunca colgar un handler esperando a un tercero
            .user_agent("rust-100-api-clima/0.1")
            .build()
            .expect("cliente http");
        OpenMeteo { http, base_url: base_url.into() }
    }
}

/// Forma (parcial) de la respuesta de Open-Meteo.
#[derive(Deserialize)]
struct RespuestaOM {
    current: ActualOM,
}
#[derive(Deserialize)]
struct ActualOM {
    temperature_2m: f64,
    wind_speed_10m: f64,
    weather_code: i64,
}

impl Proveedor for OpenMeteo {
    async fn consultar(&self, lat: f64, lon: f64) -> Result<Clima, ErrorProveedor> {
        let url = format!("{}/v1/forecast", self.base_url.trim_end_matches('/'));
        let r = self
            .http
            .get(url)
            .query(&[("latitude", lat.to_string()), ("longitude", lon.to_string()), ("current", "temperature_2m,wind_speed_10m,weather_code".into())])
            .send()
            .await
            .map_err(|e| ErrorProveedor::NoDisponible(e.to_string()))?;
        if !r.status().is_success() {
            return Err(ErrorProveedor::NoDisponible(format!("HTTP {}", r.status())));
        }
        let j: RespuestaOM = r.json().await.map_err(|e| ErrorProveedor::Formato(e.to_string()))?;
        Ok(Clima {
            temperatura_c: j.current.temperature_2m,
            viento_kmh: j.current.wind_speed_10m,
            codigo: j.current.weather_code,
            descripcion: describir_wmo(j.current.weather_code).into(),
        })
    }
}
