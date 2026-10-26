//! # Acortador de URLs
//!
//! - `POST /api/acortar` `{"url": "...", "alias": "opcional"}` → `201 {"codigo", "corta", "url"}`.
//! - `GET /{codigo}` → **307** a la URL original y suma un clic.
//! - `GET /api/stats/{codigo}` → clics, fecha de creación y último acceso.
//! - `DELETE /api/{codigo}` → borra.
//!
//! ## Códigos
//! Sin alias, el código es el **id en base62** (`0-9a-zA-Z`) más un desplazamiento
//! fijo para que no empiecen en "1", "2"... Es determinista y sin colisiones, a
//! costa de ser predecible (ver README).
//!
//! ## Seguridad
//! - Solo se aceptan URLs `http`/`https` con host (nada de `javascript:` o `file:`).
//! - Se evita acortar URLs del propio servicio (bucles de redirección).
//! - Alias restringidos a `[A-Za-z0-9_-]{3,32}` y sin palabras reservadas (`api`).
//! - Se usa 307 (no 301) para que los navegadores no cacheen y sigamos contando clics.

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::sync::Arc;

const ALFABETO: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// Desplazamiento para que los primeros códigos tengan 4 caracteres.
const DESPLAZAMIENTO: u64 = 62 * 62 * 62;

/// Codifica un número en base62.
pub fn base62(mut n: u64) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut v = Vec::new();
    while n > 0 {
        v.push(ALFABETO[(n % 62) as usize]);
        n /= 62;
    }
    v.reverse();
    String::from_utf8(v).expect("ASCII")
}

/// Valida y normaliza una URL destino.
pub fn validar_url(s: &str, base_propia: &str) -> Result<String, String> {
    let u = url::Url::parse(s.trim()).map_err(|e| format!("URL inválida: {e}"))?;
    if !matches!(u.scheme(), "http" | "https") {
        return Err("solo se aceptan URLs http/https".into());
    }
    let host = u.host_str().ok_or("la URL no tiene host")?;
    if let Ok(propia) = url::Url::parse(base_propia) {
        if propia.host_str() == Some(host) && propia.port_or_known_default() == u.port_or_known_default() {
            return Err("no se pueden acortar URLs de este mismo servicio".into());
        }
    }
    Ok(u.to_string())
}

/// Valida un alias elegido por el usuario.
pub fn validar_alias(a: &str) -> Result<(), String> {
    let ok_len = (3..=32).contains(&a.len());
    let ok_chars = a.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !ok_len || !ok_chars {
        return Err("alias: 3..=32 caracteres [A-Za-z0-9_-]".into());
    }
    if ["api", "salud", "static"].contains(&a.to_lowercase().as_str()) {
        return Err("alias reservado".into());
    }
    Ok(())
}

/// Estado compartido: pool + URL base pública (para armar la URL corta).
#[derive(Clone)]
pub struct Estado {
    pub db: SqlitePool,
    pub base: Arc<str>,
}

#[derive(Deserialize)]
struct Pedido {
    url: String,
    alias: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Stats {
    pub codigo: String,
    pub url: String,
    pub clics: i64,
    pub creado: String,
    pub ultimo_acceso: Option<String>,
}

fn error(c: StatusCode, m: impl Into<String>) -> Response {
    (c, Json(serde_json::json!({"error": m.into()}))).into_response()
}

/// Crea pool + esquema.
pub async fn crear_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let o = SqliteConnectOptions::from_str(url)?.create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(if url.contains(":memory:") { 1 } else { 5 }).connect_with(o).await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS enlaces (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            codigo TEXT UNIQUE,
            url TEXT NOT NULL,
            clics INTEGER NOT NULL DEFAULT 0,
            creado TEXT NOT NULL DEFAULT (datetime('now')),
            ultimo_acceso TEXT
        )",
    )
    .execute(&pool)
    .await?;
    Ok(pool)
}

pub fn app(estado: Estado) -> Router {
    Router::new()
        .route("/salud", get(|| async { "ok" }))
        .route("/api/acortar", post(acortar))
        .route("/api/stats/{codigo}", get(stats))
        .route("/api/{codigo}", delete(borrar))
        .route("/{codigo}", get(redirigir))
        .with_state(estado)
}

async fn acortar(State(e): State<Estado>, Json(p): Json<Pedido>) -> Response {
    let destino = match validar_url(&p.url, &e.base) {
        Ok(u) => u,
        Err(m) => return error(StatusCode::UNPROCESSABLE_ENTITY, m),
    };
    if let Some(a) = &p.alias {
        if let Err(m) = validar_alias(a) {
            return error(StatusCode::UNPROCESSABLE_ENTITY, m);
        }
    }
    let r: Result<String, sqlx::Error> = async {
        match &p.alias {
            Some(a) => {
                sqlx::query("INSERT INTO enlaces (codigo, url) VALUES (?, ?)").bind(a).bind(&destino).execute(&e.db).await?;
                Ok(a.clone())
            }
            None => {
                // Transacción: insertar sin código, calcularlo desde el id y actualizar.
                let mut tx = e.db.begin().await?;
                let id = sqlx::query("INSERT INTO enlaces (url) VALUES (?)").bind(&destino).execute(&mut *tx).await?.last_insert_rowid();
                let mut codigo = base62(id as u64 + DESPLAZAMIENTO);
                // Si un alias manual ya ocupó ese código, le agregamos sufijo.
                while sqlx::query("SELECT 1 FROM enlaces WHERE codigo = ?").bind(&codigo).fetch_optional(&mut *tx).await?.is_some() {
                    codigo.push('x');
                }
                sqlx::query("UPDATE enlaces SET codigo = ? WHERE id = ?").bind(&codigo).bind(id).execute(&mut *tx).await?;
                tx.commit().await?;
                Ok(codigo)
            }
        }
    }
    .await;
    match r {
        Ok(c) => {
            let corta = format!("{}/{}", e.base.trim_end_matches('/'), c);
            (StatusCode::CREATED, [(header::LOCATION, corta.clone())], Json(serde_json::json!({"codigo": c, "corta": corta, "url": destino}))).into_response()
        }
        Err(sqlx::Error::Database(d)) if d.is_unique_violation() => error(StatusCode::CONFLICT, "el alias ya está en uso"),
        Err(x) => error(StatusCode::INTERNAL_SERVER_ERROR, x.to_string()),
    }
}

async fn redirigir(State(e): State<Estado>, Path(codigo): Path<String>) -> Response {
    // UPDATE ... RETURNING: incrementa y obtiene la URL en una sola consulta atómica.
    let r: Result<Option<(String,)>, _> = sqlx::query_as(
        "UPDATE enlaces SET clics = clics + 1, ultimo_acceso = datetime('now') WHERE codigo = ? RETURNING url",
    )
    .bind(&codigo)
    .fetch_optional(&e.db)
    .await;
    match r {
        Ok(Some((url,))) => Redirect::temporary(&url).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "código inexistente"),
        Err(x) => error(StatusCode::INTERNAL_SERVER_ERROR, x.to_string()),
    }
}

async fn stats(State(e): State<Estado>, Path(codigo): Path<String>) -> Response {
    match sqlx::query_as::<_, Stats>("SELECT codigo, url, clics, creado, ultimo_acceso FROM enlaces WHERE codigo = ?")
        .bind(&codigo)
        .fetch_optional(&e.db)
        .await
    {
        Ok(Some(s)) => Json(s).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "código inexistente"),
        Err(x) => error(StatusCode::INTERNAL_SERVER_ERROR, x.to_string()),
    }
}

async fn borrar(State(e): State<Estado>, Path(codigo): Path<String>) -> Response {
    match sqlx::query("DELETE FROM enlaces WHERE codigo = ?").bind(&codigo).execute(&e.db).await {
        Ok(r) if r.rows_affected() > 0 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => error(StatusCode::NOT_FOUND, "código inexistente"),
        Err(x) => error(StatusCode::INTERNAL_SERVER_ERROR, x.to_string()),
    }
}
