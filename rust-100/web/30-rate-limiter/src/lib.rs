//! # Rate limiter como middleware Tower para Axum
//!
//! - [`balde`]: algoritmo token bucket puro.
//! - [`LimiteLayer`] / [`Limite`]: implementación de los traits `tower::Layer`
//!   y `tower::Service`, así que funciona con `Router::layer` (todo el router)
//!   o `route_layer`/por ruta, y con cualquier servicio Tower.
//! - La **clave** de agrupación es configurable ([`por_ip`], [`por_header`]).
//! - Respuestas `429 Too Many Requests` con `Retry-After` y cabeceras
//!   `X-RateLimit-Limit` / `X-RateLimit-Remaining` en las permitidas.
//! - **Reloj inyectable** (milisegundos) → tests deterministas.

pub mod balde;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderValue, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use balde::{Baldes, Config, Decision};
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

pub type FnClave = Arc<dyn Fn(&Request<Body>) -> Option<String> + Send + Sync>;
pub type Reloj = Arc<dyn Fn() -> u64 + Send + Sync>;

/// Clave = IP del cliente (requiere `into_make_service_with_connect_info`).
///
/// No usamos `X-Forwarded-For` por defecto: cualquiera puede falsificarlo.
/// Detrás de un proxy de confianza, usar [`por_header`] con el header que
/// ese proxy garantiza.
pub fn por_ip() -> FnClave {
    Arc::new(|r| r.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip().to_string()))
}

/// Clave = valor de un header (p. ej. `x-api-key`).
pub fn por_header(nombre: &'static str) -> FnClave {
    Arc::new(move |r| r.headers().get(nombre).and_then(|v| v.to_str().ok()).map(|s| format!("{nombre}:{s}")))
}

fn reloj_sistema() -> Reloj {
    let inicio = std::time::Instant::now();
    Arc::new(move || inicio.elapsed().as_millis() as u64)
}

/// Capa configurable. Clonarla comparte los mismos baldes.
#[derive(Clone)]
pub struct LimiteLayer {
    baldes: Arc<Mutex<Baldes>>,
    clave: FnClave,
    reloj: Reloj,
}

impl LimiteLayer {
    pub fn new(cfg: Config, clave: FnClave) -> Self {
        Self { baldes: Arc::new(Mutex::new(Baldes::new(cfg))), clave, reloj: reloj_sistema() }
    }
    pub fn con_reloj(mut self, reloj: Reloj) -> Self {
        self.reloj = reloj;
        self
    }
    /// Cuántas claves hay en memoria (para métricas/tests).
    pub fn claves(&self) -> usize {
        self.baldes.lock().expect("mutex").claves()
    }
    /// Purga baldes llenos; llamar periódicamente (ver `main`).
    pub fn purgar(&self) {
        let ahora = (self.reloj)();
        self.baldes.lock().expect("mutex").purgar(ahora);
    }
}

impl<S> tower::Layer<S> for LimiteLayer {
    type Service = Limite<S>;
    fn layer(&self, interno: S) -> Self::Service {
        Limite { interno, capa: self.clone() }
    }
}

/// El servicio que envuelve al interno.
#[derive(Clone)]
pub struct Limite<S> {
    interno: S,
    capa: LimiteLayer,
}

fn rechazo(estado: StatusCode, msg: &'static str, reintentar_ms: Option<u64>) -> Response {
    let mut r = (estado, msg).into_response();
    if let Some(ms) = reintentar_ms {
        // Retry-After va en segundos enteros: redondeamos hacia arriba (mínimo 1).
        let s = ms.div_ceil(1000).max(1);
        r.headers_mut().insert("retry-after", HeaderValue::from(s));
    }
    r
}

impl<S> tower::Service<Request<Body>> for Limite<S>
where
    S: tower::Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.interno.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // Sin clave identificable → balde compartido "anonimo" (conservador).
        let clave = (self.capa.clave)(&req).unwrap_or_else(|| "anonimo".into());
        let ahora = (self.capa.reloj)();
        let (decision, limite) = {
            // El lock se suelta al cerrar este bloque: nunca lo sostenemos
            // mientras corre el handler.
            let mut b = self.capa.baldes.lock().expect("mutex");
            (b.intentar(&clave, ahora), b.config().rafaga as u64)
        };
        match decision {
            Decision::Permitido { restantes } => {
                // Patrón Tower: usamos el clon "listo" y dejamos otro en su lugar.
                let clon = self.interno.clone();
                let mut listo = std::mem::replace(&mut self.interno, clon);
                Box::pin(async move {
                    let mut r = listo.call(req).await?;
                    let h = r.headers_mut();
                    h.insert("x-ratelimit-limit", HeaderValue::from(limite));
                    h.insert("x-ratelimit-remaining", HeaderValue::from(restantes));
                    Ok(r)
                })
            }
            Decision::Limitado { reintentar_ms } => {
                Box::pin(async move { Ok(rechazo(StatusCode::TOO_MANY_REQUESTS, "demasiadas peticiones", Some(reintentar_ms))) })
            }
            Decision::Saturado => {
                Box::pin(async move { Ok(rechazo(StatusCode::SERVICE_UNAVAILABLE, "servidor saturado", Some(1000))) })
            }
        }
    }
}

/// App de ejemplo: límite general por IP y uno **más estricto** sólo para `/login`.
pub fn app(general: LimiteLayer, login: LimiteLayer) -> Router {
    Router::new()
        .route("/login", post(|| async { "login ok" }).route_layer(login))
        .route("/", get(|| async { "hola" }))
        .route("/salud", get(|| async { "ok" }))
        .layer(general)
}
