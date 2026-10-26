//! # Webhooks: recibir, verificar, encolar y reenviar
//!
//! ```text
//!  emisor ──POST /webhooks/{fuente}──▶ [verificar HMAC + ts] ──▶ SQLite: eventos + entregas(pendiente)
//!                                                                    │
//!                     worker (cada N s) ◀────────────────────────────┘
//!                       └─ POST a cada destino (firmado con secreto de salida)
//!                            2xx → entregada | error → reintento con backoff exponencial
//!                            tras MAX_INTENTOS → fallida (dead letter, reintento manual)
//! ```
//!
//! - Respondemos **202 rápido** y reenviamos en segundo plano: el emisor no
//!   espera a nuestros destinos (y no reintenta por timeouts nuestros).
//! - **Idempotencia**: `X-Evento-Id` es único por fuente; un duplicado
//!   devuelve 200 sin re-encolar (los emisores reintentan, es normal).
//! - **Durabilidad**: la cola vive en SQLite → sobrevive a reinicios.
//! - Reloj inyectable para testear backoff sin dormir.

pub mod firma;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

pub type Reloj = Arc<dyn Fn() -> i64 + Send + Sync>;

pub struct Config {
    /// Secreto compartido con cada fuente (p. ej. "github", "pagos").
    pub secretos: HashMap<String, String>,
    /// URLs a las que reenviar los eventos de cada fuente.
    pub destinos: HashMap<String, Vec<String>>,
    /// Secreto con el que firmamos lo que reenviamos.
    pub secreto_salida: String,
    pub tolerancia_seg: i64,
    pub max_intentos: i64,
    pub backoff_base_seg: i64,
    pub max_cuerpo: usize,
}

#[derive(Clone)]
pub struct Estado {
    pub db: SqlitePool,
    pub cfg: Arc<Config>,
    pub reloj: Reloj,
    pub http: reqwest::Client,
}

impl Estado {
    pub fn new(db: SqlitePool, cfg: Config, reloj: Reloj) -> Self {
        let http = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().expect("http");
        Estado { db, cfg: Arc::new(cfg), reloj, http }
    }
}

pub async fn crear_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let o = SqliteConnectOptions::from_str(url)?.create_if_missing(true).foreign_keys(true);
    let p = SqlitePoolOptions::new().max_connections(if url.contains(":memory:") { 1 } else { 5 }).connect_with(o).await?;
    for q in [
        "CREATE TABLE IF NOT EXISTS eventos (id INTEGER PRIMARY KEY AUTOINCREMENT, fuente TEXT NOT NULL,
            evento_id TEXT NOT NULL, cuerpo BLOB NOT NULL, recibido INTEGER NOT NULL, UNIQUE(fuente, evento_id))",
        "CREATE TABLE IF NOT EXISTS entregas (id INTEGER PRIMARY KEY AUTOINCREMENT,
            evento INTEGER NOT NULL REFERENCES eventos(id) ON DELETE CASCADE, destino TEXT NOT NULL,
            estado TEXT NOT NULL DEFAULT 'pendiente', intentos INTEGER NOT NULL DEFAULT 0,
            proximo INTEGER NOT NULL, ultimo_error TEXT)",
        "CREATE INDEX IF NOT EXISTS ix_entregas_cola ON entregas(estado, proximo)",
    ] {
        sqlx::query(q).execute(&p).await?;
    }
    Ok(p)
}

pub fn app(e: Estado) -> Router {
    Router::new()
        .route("/webhooks/{fuente}", post(recibir))
        .route("/eventos/{id}/entregas", get(entregas_de))
        .route("/entregas/{id}/reintentar", post(reintentar))
        .layer(axum::extract::DefaultBodyLimit::max(e.cfg.max_cuerpo))
        .with_state(e)
}

fn error(c: StatusCode, m: &str) -> Response {
    (c, Json(serde_json::json!({"error": m}))).into_response()
}

fn interno(e: sqlx::Error) -> Response {
    eprintln!("db: {e}");
    error(StatusCode::INTERNAL_SERVER_ERROR, "error interno")
}

async fn recibir(State(e): State<Estado>, Path(fuente): Path<String>, h: HeaderMap, cuerpo: Bytes) -> Response {
    // Fuente desconocida → 404 (no revelamos si existe vía 401).
    let Some(secreto) = e.cfg.secretos.get(&fuente) else { return error(StatusCode::NOT_FOUND, "fuente desconocida") };
    let hdr = |n: &str| h.get(n).and_then(|v| v.to_str().ok());
    let (Some(ts), Some(f), Some(evento_id)) = (hdr("x-timestamp").and_then(|t| t.parse::<i64>().ok()), hdr("x-firma"), hdr("x-evento-id")) else {
        return error(StatusCode::BAD_REQUEST, "faltan X-Timestamp, X-Firma o X-Evento-Id");
    };
    // Verificamos sobre los BYTES crudos: re-serializar el JSON cambiaría la firma.
    if let Err(r) = firma::verificar(secreto.as_bytes(), ts, &cuerpo, f, (e.reloj)(), e.cfg.tolerancia_seg) {
        return error(StatusCode::UNAUTHORIZED, &format!("{r:?}"));
    }
    if evento_id.is_empty() || evento_id.len() > 128 {
        return error(StatusCode::BAD_REQUEST, "X-Evento-Id inválido");
    }
    let ahora = (e.reloj)();
    // Transacción: evento + todas sus entregas, o nada.
    let res: Result<Option<i64>, sqlx::Error> = async {
        let mut tx = e.db.begin().await?;
        let id: Option<(i64,)> = sqlx::query_as("INSERT INTO eventos (fuente, evento_id, cuerpo, recibido) VALUES (?, ?, ?, ?) ON CONFLICT DO NOTHING RETURNING id")
            .bind(&fuente)
            .bind(evento_id)
            .bind(cuerpo.as_ref())
            .bind(ahora)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((id,)) = id else { return Ok(None) }; // duplicado
        for d in e.cfg.destinos.get(&fuente).into_iter().flatten() {
            sqlx::query("INSERT INTO entregas (evento, destino, proximo) VALUES (?, ?, ?)").bind(id).bind(d).bind(ahora).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(Some(id))
    }
    .await;
    match res {
        Ok(Some(id)) => (StatusCode::ACCEPTED, Json(serde_json::json!({"id": id}))).into_response(),
        Ok(None) => (StatusCode::OK, Json(serde_json::json!({"duplicado": true}))).into_response(),
        Err(err) => interno(err),
    }
}

#[derive(Serialize, sqlx::FromRow, Debug)]
pub struct Entrega {
    pub id: i64,
    pub destino: String,
    pub estado: String,
    pub intentos: i64,
    pub proximo: i64,
    pub ultimo_error: Option<String>,
}

async fn entregas_de(State(e): State<Estado>, Path(id): Path<i64>) -> Response {
    match sqlx::query_as::<_, Entrega>("SELECT id, destino, estado, intentos, proximo, ultimo_error FROM entregas WHERE evento = ? ORDER BY id").bind(id).fetch_all(&e.db).await {
        Ok(v) => Json(v).into_response(),
        Err(err) => interno(err),
    }
}

async fn reintentar(State(e): State<Estado>, Path(id): Path<i64>) -> Response {
    let r = sqlx::query("UPDATE entregas SET estado = 'pendiente', intentos = 0, proximo = ? WHERE id = ? AND estado = 'fallida'").bind((e.reloj)()).bind(id).execute(&e.db).await;
    match r {
        Ok(r) if r.rows_affected() == 1 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => error(StatusCode::CONFLICT, "solo se reintentan entregas fallidas"),
        Err(err) => interno(err),
    }
}

/// Procesa las entregas vencidas una vez. Devuelve cuántas se intentaron.
/// Pensado para un **único** worker (con varios, habría que "reclamar" filas
/// con `UPDATE … RETURNING` antes de enviar).
pub async fn procesar_pendientes(e: &Estado) -> Result<usize, sqlx::Error> {
    let ahora = (e.reloj)();
    let filas: Vec<(i64, String, i64, String, Vec<u8>)> = sqlx::query_as(
        "SELECT en.id, en.destino, en.intentos, ev.evento_id, ev.cuerpo FROM entregas en JOIN eventos ev ON ev.id = en.evento
         WHERE en.estado = 'pendiente' AND en.proximo <= ? ORDER BY en.proximo LIMIT 50",
    )
    .bind(ahora)
    .fetch_all(&e.db)
    .await?;
    for (id, destino, intentos, evento_id, cuerpo) in &filas {
        let ts = (e.reloj)();
        let resultado = e
            .http
            .post(destino)
            .header("content-type", "application/json")
            .header("x-evento-id", evento_id)
            .header("x-timestamp", ts.to_string())
            .header("x-firma", firma::firmar(e.cfg.secreto_salida.as_bytes(), ts, cuerpo))
            .body(cuerpo.clone())
            .send()
            .await;
        let error = match resultado {
            Ok(r) if r.status().is_success() => None,
            Ok(r) => Some(format!("HTTP {}", r.status())),
            Err(err) => Some(format!("red: {err}")),
        };
        match error {
            None => {
                sqlx::query("UPDATE entregas SET estado = 'entregada', intentos = intentos + 1, ultimo_error = NULL WHERE id = ?").bind(id).execute(&e.db).await?;
            }
            Some(msg) => {
                let n = intentos + 1;
                let estado = if n >= e.cfg.max_intentos { "fallida" } else { "pendiente" };
                // Backoff exponencial: base · 2^(n-1) → 1, 2, 4, 8… × base (tope 1 h).
                let espera = (e.cfg.backoff_base_seg << (n - 1).min(20)).min(3600);
                sqlx::query("UPDATE entregas SET estado = ?, intentos = ?, proximo = ?, ultimo_error = ? WHERE id = ?")
                    .bind(estado)
                    .bind(n)
                    .bind(ahora + espera)
                    .bind(msg)
                    .bind(id)
                    .execute(&e.db)
                    .await?;
            }
        }
    }
    Ok(filas.len())
}
