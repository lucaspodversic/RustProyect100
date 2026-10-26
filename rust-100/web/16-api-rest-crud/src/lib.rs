//! # API REST CRUD — Axum + SQLx (SQLite)
//!
//! Recurso `productos` con las 5 operaciones clásicas:
//!
//! | Método | Ruta              | Acción                                   |
//! |--------|-------------------|------------------------------------------|
//! | GET    | `/productos`      | listar (`?q=`, `?limite=`, `?desplazamiento=`) |
//! | GET    | `/productos/{id}` | obtener uno                              |
//! | POST   | `/productos`      | crear → 201 + `Location`                 |
//! | PUT    | `/productos/{id}` | reemplazar                               |
//! | DELETE | `/productos/{id}` | borrar → 204                             |
//!
//! ## Arquitectura
//! - [`app`] arma el `Router` recibiendo el pool: inyección de dependencias simple,
//!   los tests pasan una base SQLite **en memoria**.
//! - [`ErrorApi`] centraliza errores y los convierte a JSON `{"error": ...}` con el
//!   código HTTP correcto (implementa `IntoResponse`).
//! - Validación explícita en [`NuevoProducto::validar`].
//! - Consultas con `sqlx::query_as` y parámetros `?` (nunca concatenando strings → sin SQL injection).

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

/// Fila de la tabla `productos`.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
pub struct Producto {
    pub id: i64,
    pub nombre: String,
    pub precio_centavos: i64,
    pub stock: i64,
}

/// Cuerpo de POST/PUT.
#[derive(Debug, Deserialize)]
pub struct NuevoProducto {
    pub nombre: String,
    pub precio_centavos: i64,
    #[serde(default)]
    pub stock: i64,
}

impl NuevoProducto {
    /// Reglas de negocio mínimas.
    pub fn validar(&self) -> Result<(), ErrorApi> {
        let n = self.nombre.trim();
        if n.is_empty() || n.chars().count() > 100 {
            return Err(ErrorApi::Validacion("nombre debe tener 1..=100 caracteres".into()));
        }
        if self.precio_centavos < 0 {
            return Err(ErrorApi::Validacion("precio_centavos no puede ser negativo".into()));
        }
        if self.stock < 0 {
            return Err(ErrorApi::Validacion("stock no puede ser negativo".into()));
        }
        Ok(())
    }
}

/// Parámetros de listado.
#[derive(Debug, Deserialize)]
pub struct Listado {
    pub q: Option<String>,
    pub limite: Option<i64>,
    pub desplazamiento: Option<i64>,
}

/// Errores de la API mapeados a HTTP.
#[derive(Debug)]
pub enum ErrorApi {
    NoEncontrado,
    Validacion(String),
    Conflicto(String),
    Interno(String),
}

impl IntoResponse for ErrorApi {
    fn into_response(self) -> Response {
        let (codigo, msg) = match self {
            ErrorApi::NoEncontrado => (StatusCode::NOT_FOUND, "no encontrado".to_string()),
            ErrorApi::Validacion(m) => (StatusCode::UNPROCESSABLE_ENTITY, m),
            ErrorApi::Conflicto(m) => (StatusCode::CONFLICT, m),
            // No filtramos detalles internos al cliente; se loguean en el servidor.
            ErrorApi::Interno(m) => {
                eprintln!("error interno: {m}");
                (StatusCode::INTERNAL_SERVER_ERROR, "error interno".to_string())
            }
        };
        (codigo, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

impl From<sqlx::Error> for ErrorApi {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::RowNotFound => ErrorApi::NoEncontrado,
            // Violación de UNIQUE → 409.
            sqlx::Error::Database(d) if d.is_unique_violation() => ErrorApi::Conflicto("ya existe un producto con ese nombre".into()),
            _ => ErrorApi::Interno(e.to_string()),
        }
    }
}

/// Crea el pool y aplica el esquema. `url` ej.: `sqlite::memory:` o `sqlite://datos.db`.
///
/// Con `:memory:` cada conexión tendría su **propia** base, por eso se limita a 1 conexión.
pub async fn crear_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let opciones = SqliteConnectOptions::from_str(url)?.create_if_missing(true).foreign_keys(true);
    let max = if url.contains(":memory:") { 1 } else { 5 };
    let pool = SqlitePoolOptions::new().max_connections(max).connect_with(opciones).await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS productos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nombre TEXT NOT NULL UNIQUE,
            precio_centavos INTEGER NOT NULL CHECK (precio_centavos >= 0),
            stock INTEGER NOT NULL DEFAULT 0 CHECK (stock >= 0)
        )",
    )
    .execute(&pool)
    .await?;
    Ok(pool)
}

/// Router de la aplicación.
pub fn app(pool: SqlitePool) -> Router {
    Router::new()
        .route("/salud", get(|| async { "ok" }))
        .route("/productos", get(listar).post(crear))
        .route("/productos/{id}", get(obtener).put(reemplazar).delete(borrar))
        .with_state(pool)
}

async fn listar(State(db): State<SqlitePool>, Query(p): Query<Listado>) -> Result<Json<Vec<Producto>>, ErrorApi> {
    // Límite acotado para que nadie pida un millón de filas.
    let limite = p.limite.unwrap_or(20).clamp(1, 100);
    let desde = p.desplazamiento.unwrap_or(0).max(0);
    // El filtro LIKE va parametrizado; los comodines del usuario se escapan.
    let patron = format!("%{}%", p.q.unwrap_or_default().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let v = sqlx::query_as::<_, Producto>(
        "SELECT id, nombre, precio_centavos, stock FROM productos
         WHERE nombre LIKE ? ESCAPE '\\' ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(patron)
    .bind(limite)
    .bind(desde)
    .fetch_all(&db)
    .await?;
    Ok(Json(v))
}

async fn obtener(State(db): State<SqlitePool>, Path(id): Path<i64>) -> Result<Json<Producto>, ErrorApi> {
    let p = sqlx::query_as::<_, Producto>("SELECT id, nombre, precio_centavos, stock FROM productos WHERE id = ?")
        .bind(id)
        .fetch_one(&db)
        .await?;
    Ok(Json(p))
}

async fn crear(State(db): State<SqlitePool>, Json(n): Json<NuevoProducto>) -> Result<impl IntoResponse, ErrorApi> {
    n.validar()?;
    let p = sqlx::query_as::<_, Producto>(
        "INSERT INTO productos (nombre, precio_centavos, stock) VALUES (?, ?, ?)
         RETURNING id, nombre, precio_centavos, stock",
    )
    .bind(n.nombre.trim())
    .bind(n.precio_centavos)
    .bind(n.stock)
    .fetch_one(&db)
    .await?;
    let ubicacion = format!("/productos/{}", p.id);
    Ok((StatusCode::CREATED, [(header::LOCATION, ubicacion)], Json(p)))
}

async fn reemplazar(State(db): State<SqlitePool>, Path(id): Path<i64>, Json(n): Json<NuevoProducto>) -> Result<Json<Producto>, ErrorApi> {
    n.validar()?;
    let p = sqlx::query_as::<_, Producto>(
        "UPDATE productos SET nombre = ?, precio_centavos = ?, stock = ? WHERE id = ?
         RETURNING id, nombre, precio_centavos, stock",
    )
    .bind(n.nombre.trim())
    .bind(n.precio_centavos)
    .bind(n.stock)
    .bind(id)
    .fetch_one(&db) // sin filas → RowNotFound → 404
    .await?;
    Ok(Json(p))
}

async fn borrar(State(db): State<SqlitePool>, Path(id): Path<i64>) -> Result<StatusCode, ErrorApi> {
    let r = sqlx::query("DELETE FROM productos WHERE id = ?").bind(id).execute(&db).await?;
    if r.rows_affected() == 0 {
        return Err(ErrorApi::NoEncontrado);
    }
    Ok(StatusCode::NO_CONTENT)
}
