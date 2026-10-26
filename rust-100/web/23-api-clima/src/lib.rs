//! # API de clima con caché
//!
//! `GET /clima?lat=-34.6&lon=-58.4` → `{clima, origen: "cache"|"proveedor"|"cache-vencida"}`
//! `GET /metricas` → aciertos/fallos de caché y llamadas al proveedor.
//!
//! Flujo:
//! 1. Validar coordenadas (lat ∈ [-90, 90], lon ∈ [-180, 180]) → 400.
//! 2. Caché fresca → responder sin red.
//! 3. Si no, preguntar al proveedor y guardar.
//! 4. Si el proveedor falla y hay entrada vencida → servirla con
//!    `origen: "cache-vencida"` (degradación elegante); si no → 502/503.

pub mod cache;
pub mod proveedor;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use cache::{clave, Busqueda, Cache, Reloj};
use proveedor::{Clima, ErrorProveedor, Proveedor};
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub struct Estado<P: Proveedor, R: Reloj> {
    pub proveedor: P,
    pub cache: Cache<Clima>,
    pub reloj: R,
    aciertos: AtomicU64,
    fallos: AtomicU64,
    llamadas: AtomicU64,
}

impl<P: Proveedor, R: Reloj> Estado<P, R> {
    pub fn new(proveedor: P, cache: Cache<Clima>, reloj: R) -> Arc<Self> {
        Arc::new(Estado { proveedor, cache, reloj, aciertos: 0.into(), fallos: 0.into(), llamadas: 0.into() })
    }
}

pub fn app<P: Proveedor, R: Reloj>(e: Arc<Estado<P, R>>) -> Router {
    Router::new().route("/clima", get(clima::<P, R>)).route("/metricas", get(metricas::<P, R>)).with_state(e)
}

#[derive(Deserialize)]
struct Coords {
    lat: f64,
    lon: f64,
}

fn error(c: StatusCode, m: &str) -> Response {
    (c, Json(serde_json::json!({"error": m}))).into_response()
}

async fn clima<P: Proveedor, R: Reloj>(State(e): State<Arc<Estado<P, R>>>, Query(c): Query<Coords>) -> Response {
    if !(-90.0..=90.0).contains(&c.lat) || !(-180.0..=180.0).contains(&c.lon) {
        return error(StatusCode::BAD_REQUEST, "coordenadas fuera de rango");
    }
    let k = clave(c.lat, c.lon);
    let vencido = match e.cache.buscar(k, e.reloj.ahora()) {
        Busqueda::Fresco(v) => {
            e.aciertos.fetch_add(1, Ordering::Relaxed);
            return Json(serde_json::json!({"clima": v, "origen": "cache"})).into_response();
        }
        Busqueda::Vencido(v) => Some(v),
        Busqueda::Nada => None,
    };
    e.fallos.fetch_add(1, Ordering::Relaxed);
    e.llamadas.fetch_add(1, Ordering::Relaxed);
    // Consultamos con las coordenadas redondeadas (las de la clave) para que
    // lo cacheado corresponda exactamente a la clave.
    match e.proveedor.consultar(k.0 as f64 / 100.0, k.1 as f64 / 100.0).await {
        Ok(v) => {
            e.cache.guardar(k, v.clone(), e.reloj.ahora());
            Json(serde_json::json!({"clima": v, "origen": "proveedor"})).into_response()
        }
        Err(err) => {
            eprintln!("proveedor falló: {err:?}");
            match (vencido, err) {
                (Some(v), _) => Json(serde_json::json!({"clima": v, "origen": "cache-vencida"})).into_response(),
                (None, ErrorProveedor::NoDisponible(_)) => error(StatusCode::SERVICE_UNAVAILABLE, "servicio de clima no disponible"),
                (None, ErrorProveedor::Formato(_)) => error(StatusCode::BAD_GATEWAY, "respuesta inválida del servicio de clima"),
            }
        }
    }
}

async fn metricas<P: Proveedor, R: Reloj>(State(e): State<Arc<Estado<P, R>>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "aciertos": e.aciertos.load(Ordering::Relaxed),
        "fallos": e.fallos.load(Ordering::Relaxed),
        "llamadas_proveedor": e.llamadas.load(Ordering::Relaxed),
        "entradas": e.cache.len(),
    }))
}
