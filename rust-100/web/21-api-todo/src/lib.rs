//! # API To-Do con listas e ítems
//!
//! Recursos **anidados**:
//!
//! | Método | Ruta | Descripción |
//! |---|---|---|
//! | GET/POST | `/listas` | listas con conteo total/pendientes |
//! | GET/DELETE | `/listas/{id}` | una lista (con ítems) / borrar en cascada |
//! | GET/POST | `/listas/{id}/items?completado=` | ítems de la lista |
//! | PATCH/DELETE | `/listas/{id}/items/{item}` | actualización **parcial** / borrar |
//! | POST | `/listas/{id}/completar-todo` | marca todos como hechos |
//!
//! ## PATCH parcial
//! [`CambioItem`] usa `Option<T>` en cada campo: lo ausente no se toca.
//! En SQL: `SET titulo = COALESCE(?, titulo)`. Para `vence` (que puede querer
//! borrarse) se usa `Option<Option<String>>`: ausente = no tocar,
//! `null` = borrar, `"2026-01-01"` = fijar.
//!
//! ## Integridad
//! Cada ítem se busca **junto con su lista** (`WHERE id = ? AND lista_id = ?`),
//! así `/listas/1/items/99` no puede modificar un ítem de otra lista.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ResumenLista {
    pub id: i64,
    pub nombre: String,
    pub total: i64,
    pub pendientes: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow, PartialEq)]
pub struct Item {
    pub id: i64,
    pub lista_id: i64,
    pub titulo: String,
    pub completado: bool,
    pub vence: Option<String>,
    pub completado_en: Option<String>,
}

#[derive(Deserialize)]
pub struct NuevaLista {
    pub nombre: String,
}

#[derive(Deserialize)]
pub struct NuevoItem {
    pub titulo: String,
    pub vence: Option<String>,
}

/// Distingue "campo ausente" (None) de "campo = null" (Some(None)).
fn doble_option<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Ok(Some(Option::deserialize(d)?))
}

#[derive(Deserialize, Default)]
pub struct CambioItem {
    pub titulo: Option<String>,
    pub completado: Option<bool>,
    #[serde(default, deserialize_with = "doble_option")]
    pub vence: Option<Option<String>>,
}

pub enum ErrorApi {
    NoEncontrado,
    Invalido(String),
    Db(sqlx::Error),
}
impl From<sqlx::Error> for ErrorApi {
    fn from(e: sqlx::Error) -> Self {
        ErrorApi::Db(e)
    }
}
impl IntoResponse for ErrorApi {
    fn into_response(self) -> Response {
        let (c, m) = match self {
            ErrorApi::NoEncontrado => (StatusCode::NOT_FOUND, "no encontrado".into()),
            ErrorApi::Invalido(m) => (StatusCode::UNPROCESSABLE_ENTITY, m),
            ErrorApi::Db(e) => {
                eprintln!("db: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "error interno".into())
            }
        };
        (c, Json(serde_json::json!({"error": m}))).into_response()
    }
}
type Res<T> = Result<T, ErrorApi>;

/// Valida fecha `AAAA-MM-DD` (formato y rangos básicos, incluye bisiestos).
pub fn fecha_valida(s: &str) -> bool {
    let p: Vec<&str> = s.split('-').collect();
    if p.len() != 3 || p[0].len() != 4 || p[1].len() != 2 || p[2].len() != 2 {
        return false;
    }
    let (Ok(a), Ok(m), Ok(d)) = (p[0].parse::<u32>(), p[1].parse::<u32>(), p[2].parse::<u32>()) else { return false };
    let bisiesto = (a % 4 == 0 && a % 100 != 0) || a % 400 == 0;
    let dias = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if bisiesto => 29,
        2 => 28,
        _ => return false,
    };
    (1..=dias).contains(&d)
}

fn validar_titulo(t: &str) -> Res<()> {
    if t.trim().is_empty() || t.chars().count() > 200 {
        return Err(ErrorApi::Invalido("título requerido (máx. 200)".into()));
    }
    Ok(())
}

pub async fn crear_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let o = SqliteConnectOptions::from_str(url)?.create_if_missing(true).foreign_keys(true);
    let pool = SqlitePoolOptions::new().max_connections(if url.contains(":memory:") { 1 } else { 5 }).connect_with(o).await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS listas (id INTEGER PRIMARY KEY AUTOINCREMENT, nombre TEXT NOT NULL)").execute(&pool).await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY AUTOINCREMENT,
            lista_id INTEGER NOT NULL REFERENCES listas(id) ON DELETE CASCADE,
            titulo TEXT NOT NULL, completado BOOLEAN NOT NULL DEFAULT 0,
            vence TEXT, completado_en TEXT)",
    )
    .execute(&pool)
    .await?;
    Ok(pool)
}

pub fn app(db: SqlitePool) -> Router {
    Router::new()
        .route("/listas", get(listas).post(crear_lista))
        .route("/listas/{id}", get(una_lista).delete(borrar_lista))
        .route("/listas/{id}/items", get(items).post(crear_item))
        .route("/listas/{id}/items/{item}", patch(cambiar_item).delete(borrar_item))
        .route("/listas/{id}/completar-todo", post(completar_todo))
        .with_state(db)
}

const SQL_RESUMEN: &str = "SELECT l.id, l.nombre, COUNT(i.id) AS total,
    COALESCE(SUM(CASE WHEN i.completado = 0 THEN 1 ELSE 0 END), 0) AS pendientes
    FROM listas l LEFT JOIN items i ON i.lista_id = l.id";

async fn listas(State(db): State<SqlitePool>) -> Res<Json<Vec<ResumenLista>>> {
    Ok(Json(sqlx::query_as(&format!("{SQL_RESUMEN} GROUP BY l.id ORDER BY l.id")).fetch_all(&db).await?))
}

async fn crear_lista(State(db): State<SqlitePool>, Json(n): Json<NuevaLista>) -> Res<(StatusCode, Json<ResumenLista>)> {
    validar_titulo(&n.nombre)?;
    let id = sqlx::query("INSERT INTO listas (nombre) VALUES (?)").bind(n.nombre.trim()).execute(&db).await?.last_insert_rowid();
    Ok((StatusCode::CREATED, Json(ResumenLista { id, nombre: n.nombre.trim().into(), total: 0, pendientes: 0 })))
}

async fn existe_lista(db: &SqlitePool, id: i64) -> Res<()> {
    sqlx::query("SELECT 1 FROM listas WHERE id = ?").bind(id).fetch_optional(db).await?.map(|_| ()).ok_or(ErrorApi::NoEncontrado)
}

async fn una_lista(State(db): State<SqlitePool>, Path(id): Path<i64>) -> Res<Json<serde_json::Value>> {
    let r: ResumenLista = sqlx::query_as(&format!("{SQL_RESUMEN} WHERE l.id = ? GROUP BY l.id"))
        .bind(id)
        .fetch_optional(&db)
        .await?
        .ok_or(ErrorApi::NoEncontrado)?;
    let its: Vec<Item> = sqlx::query_as("SELECT * FROM items WHERE lista_id = ? ORDER BY completado, vence IS NULL, vence, id").bind(id).fetch_all(&db).await?;
    Ok(Json(serde_json::json!({"lista": r, "items": its})))
}

async fn borrar_lista(State(db): State<SqlitePool>, Path(id): Path<i64>) -> Res<StatusCode> {
    let n = sqlx::query("DELETE FROM listas WHERE id = ?").bind(id).execute(&db).await?.rows_affected();
    if n == 0 { Err(ErrorApi::NoEncontrado) } else { Ok(StatusCode::NO_CONTENT) }
}

#[derive(Deserialize)]
struct FiltroItems {
    completado: Option<bool>,
}

async fn items(State(db): State<SqlitePool>, Path(id): Path<i64>, Query(f): Query<FiltroItems>) -> Res<Json<Vec<Item>>> {
    existe_lista(&db, id).await?;
    let v = sqlx::query_as("SELECT * FROM items WHERE lista_id = ? AND (? IS NULL OR completado = ?) ORDER BY id")
        .bind(id)
        .bind(f.completado)
        .bind(f.completado)
        .fetch_all(&db)
        .await?;
    Ok(Json(v))
}

async fn crear_item(State(db): State<SqlitePool>, Path(id): Path<i64>, Json(n): Json<NuevoItem>) -> Res<(StatusCode, Json<Item>)> {
    validar_titulo(&n.titulo)?;
    if n.vence.as_deref().is_some_and(|f| !fecha_valida(f)) {
        return Err(ErrorApi::Invalido("vence debe ser AAAA-MM-DD válida".into()));
    }
    existe_lista(&db, id).await?;
    let it = sqlx::query_as("INSERT INTO items (lista_id, titulo, vence) VALUES (?, ?, ?) RETURNING *")
        .bind(id)
        .bind(n.titulo.trim())
        .bind(n.vence)
        .fetch_one(&db)
        .await?;
    Ok((StatusCode::CREATED, Json(it)))
}

async fn cambiar_item(State(db): State<SqlitePool>, Path((id, item)): Path<(i64, i64)>, Json(c): Json<CambioItem>) -> Res<Json<Item>> {
    if let Some(t) = &c.titulo {
        validar_titulo(t)?;
    }
    if let Some(Some(f)) = &c.vence {
        if !fecha_valida(f) {
            return Err(ErrorApi::Invalido("vence debe ser AAAA-MM-DD válida".into()));
        }
    }
    let (tocar_vence, nuevo_vence) = match c.vence {
        None => (false, None),
        Some(v) => (true, v),
    };
    // completado_en se fija al completar y se limpia al des-completar.
    let it = sqlx::query_as(
        "UPDATE items SET
            titulo = COALESCE(?, titulo),
            completado_en = CASE WHEN ? IS NULL THEN completado_en
                                 WHEN ? = 1 AND completado = 0 THEN datetime('now')
                                 WHEN ? = 0 THEN NULL ELSE completado_en END,
            completado = COALESCE(?, completado),
            vence = CASE WHEN ? THEN ? ELSE vence END
         WHERE id = ? AND lista_id = ? RETURNING *",
    )
    .bind(c.titulo.as_deref().map(str::trim))
    .bind(c.completado)
    .bind(c.completado)
    .bind(c.completado)
    .bind(c.completado)
    .bind(tocar_vence)
    .bind(nuevo_vence)
    .bind(item)
    .bind(id)
    .fetch_optional(&db)
    .await?
    .ok_or(ErrorApi::NoEncontrado)?;
    Ok(Json(it))
}

async fn borrar_item(State(db): State<SqlitePool>, Path((id, item)): Path<(i64, i64)>) -> Res<StatusCode> {
    let n = sqlx::query("DELETE FROM items WHERE id = ? AND lista_id = ?").bind(item).bind(id).execute(&db).await?.rows_affected();
    if n == 0 { Err(ErrorApi::NoEncontrado) } else { Ok(StatusCode::NO_CONTENT) }
}

async fn completar_todo(State(db): State<SqlitePool>, Path(id): Path<i64>) -> Res<Json<serde_json::Value>> {
    existe_lista(&db, id).await?;
    let n = sqlx::query("UPDATE items SET completado = 1, completado_en = datetime('now') WHERE lista_id = ? AND completado = 0")
        .bind(id)
        .execute(&db)
        .await?
        .rows_affected();
    Ok(Json(serde_json::json!({"completados": n})))
}
