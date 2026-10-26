//! # Backend de blog
//!
//! API JSON para un blog con **posts**, **comentarios** (1-N) y **etiquetas** (N-M).
//!
//! | Método | Ruta | Descripción |
//! |---|---|---|
//! | GET | `/posts?etiqueta=&borradores=true` | listar (publicados por defecto) |
//! | POST | `/posts` | crear (slug automático y único) |
//! | GET | `/posts/{slug}` | detalle con etiquetas y comentarios |
//! | PUT | `/posts/{slug}` | actualizar (slug estable) |
//! | DELETE | `/posts/{slug}` | borrar (comentarios en cascada) |
//! | POST | `/posts/{slug}/comentarios` | comentar (solo publicados) |
//! | GET | `/etiquetas` | nube de etiquetas con conteo |
//!
//! Capas: `repo` (SQL) ← handlers (HTTP). Los errores de SQLx se mapean a 500
//! sin filtrar detalles; validaciones → 422; inexistentes → 404.

pub mod repo;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use repo::DatosPost;
use serde::Deserialize;
use sqlx::SqlitePool;

/// Error de la API.
pub enum ErrorApi {
    NoEncontrado,
    Invalido(&'static str),
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
            ErrorApi::NoEncontrado => (StatusCode::NOT_FOUND, "no encontrado".to_string()),
            ErrorApi::Invalido(m) => (StatusCode::UNPROCESSABLE_ENTITY, m.to_string()),
            ErrorApi::Db(e) => {
                eprintln!("db: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "error interno".to_string())
            }
        };
        (c, Json(serde_json::json!({"error": m}))).into_response()
    }
}

type Res<T> = Result<T, ErrorApi>;

pub fn app(db: SqlitePool) -> Router {
    Router::new()
        .route("/posts", get(listar).post(crear))
        .route("/posts/{slug}", get(detalle).put(actualizar).delete(borrar))
        .route("/posts/{slug}/comentarios", axum::routing::post(comentar))
        .route("/etiquetas", get(etiquetas))
        .with_state(db)
}

fn validar(d: &DatosPost) -> Res<()> {
    if d.titulo.trim().is_empty() || d.titulo.len() > 200 {
        return Err(ErrorApi::Invalido("título requerido (máx. 200)"));
    }
    if d.cuerpo.trim().is_empty() {
        return Err(ErrorApi::Invalido("cuerpo requerido"));
    }
    if d.etiquetas.len() > 10 {
        return Err(ErrorApi::Invalido("máximo 10 etiquetas"));
    }
    Ok(())
}

#[derive(Deserialize)]
struct FiltroListado {
    etiqueta: Option<String>,
    #[serde(default)]
    borradores: bool,
}

async fn listar(State(db): State<SqlitePool>, Query(f): Query<FiltroListado>) -> Res<Json<Vec<repo::Post>>> {
    Ok(Json(repo::listar(&db, !f.borradores, f.etiqueta.as_deref()).await?))
}

async fn crear(State(db): State<SqlitePool>, Json(d): Json<DatosPost>) -> Res<(StatusCode, Json<repo::Post>)> {
    validar(&d)?;
    Ok((StatusCode::CREATED, Json(repo::crear_post(&db, &d).await?)))
}

async fn detalle(State(db): State<SqlitePool>, Path(slug): Path<String>) -> Res<Json<repo::PostCompleto>> {
    repo::obtener(&db, &slug).await?.map(Json).ok_or(ErrorApi::NoEncontrado)
}

async fn actualizar(State(db): State<SqlitePool>, Path(slug): Path<String>, Json(d): Json<DatosPost>) -> Res<Json<repo::Post>> {
    validar(&d)?;
    repo::actualizar_post(&db, &slug, &d).await?.map(Json).ok_or(ErrorApi::NoEncontrado)
}

async fn borrar(State(db): State<SqlitePool>, Path(slug): Path<String>) -> Res<StatusCode> {
    if repo::borrar_post(&db, &slug).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ErrorApi::NoEncontrado)
    }
}

#[derive(Deserialize)]
struct NuevoComentario {
    autor: String,
    texto: String,
}

async fn comentar(State(db): State<SqlitePool>, Path(slug): Path<String>, Json(c): Json<NuevoComentario>) -> Res<(StatusCode, Json<repo::Comentario>)> {
    if c.autor.trim().is_empty() || c.texto.trim().is_empty() || c.texto.len() > 2000 {
        return Err(ErrorApi::Invalido("autor y texto requeridos (texto máx. 2000)"));
    }
    let r = repo::comentar(&db, &slug, &c.autor, &c.texto).await?.ok_or(ErrorApi::NoEncontrado)?;
    Ok((StatusCode::CREATED, Json(r)))
}

async fn etiquetas(State(db): State<SqlitePool>) -> Res<Json<serde_json::Value>> {
    let v = repo::nube_etiquetas(&db).await?;
    Ok(Json(serde_json::json!(v.into_iter().map(|(n, c)| serde_json::json!({"nombre": n, "posts": c})).collect::<Vec<_>>())))
}
