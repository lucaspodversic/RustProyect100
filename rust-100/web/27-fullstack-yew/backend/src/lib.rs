//! # Backend del full-stack
//!
//! - `GET /` → HTML con la app **ya renderizada** por Yew (SSR) + las notas
//!   en JSON dentro de `<script id="datos">` para que el WASM hidrate con
//!   el mismo estado.
//! - `GET/POST /api/notas`, `POST /api/notas/{id}/alternar`, `DELETE /api/notas/{id}`.
//! - `/pkg/*` → archivos del frontend compilado a WASM (si existen).
//!
//! La validación usa `compartido::validar_texto`, la MISMA que el frontend.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use compartido::{validar_texto, Nota, NuevaNota};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct Datos {
    notas: Vec<Nota>,
    siguiente: u32,
}

pub type Estado = Arc<Mutex<Datos>>;

pub fn app(e: Estado, dir_pkg: &str) -> Router {
    Router::new()
        .route("/", get(inicio))
        .route("/api/notas", get(listar).post(crear))
        .route("/api/notas/{id}/alternar", post(alternar))
        .route("/api/notas/{id}", delete(borrar))
        .nest_service("/pkg", tower_http::services::ServeDir::new(dir_pkg))
        .with_state(e)
}

/// JSON seguro para incrustar dentro de `<script>`: escapamos `<` para que
/// un texto como `</script>` no cierre la etiqueta (XSS).
pub fn json_para_script(v: &impl serde::Serialize) -> String {
    serde_json::to_string(v).expect("json").replace('<', "\\u003c")
}

async fn inicio(State(e): State<Estado>) -> Html<String> {
    let notas = e.lock().expect("mutex").notas.clone();
    let datos = json_para_script(&notas);
    let cuerpo = frontend::renderizar(notas).await;
    Html(format!(
        r#"<!doctype html>
<html lang="es"><head><meta charset="utf-8"><title>Notas full-stack</title>
<style>body{{font-family:sans-serif;max-width:600px;margin:2em auto}} .hecha span{{text-decoration:line-through}} .error{{color:#b00}} .activo{{font-weight:bold}}</style>
</head><body>{cuerpo}
<script id="datos" type="application/json">{datos}</script>
<script type="module">import init from '/pkg/frontend.js'; init().catch(() => console.info('WASM no compilado: página solo SSR'));</script>
</body></html>"#
    ))
}

async fn listar(State(e): State<Estado>) -> Json<Vec<Nota>> {
    Json(e.lock().expect("mutex").notas.clone())
}

async fn crear(State(e): State<Estado>, Json(n): Json<NuevaNota>) -> Response {
    match validar_texto(&n.texto) {
        Err(m) => (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error": m}))).into_response(),
        Ok(texto) => {
            let mut d = e.lock().expect("mutex");
            d.siguiente += 1;
            let nota = Nota { id: d.siguiente, texto, hecha: false };
            d.notas.push(nota.clone());
            (StatusCode::CREATED, Json(nota)).into_response()
        }
    }
}

async fn alternar(State(e): State<Estado>, Path(id): Path<u32>) -> Response {
    let mut d = e.lock().expect("mutex");
    match d.notas.iter_mut().find(|n| n.id == id) {
        Some(n) => {
            n.hecha = !n.hecha;
            Json(n.clone()).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn borrar(State(e): State<Estado>, Path(id): Path<u32>) -> StatusCode {
    let mut d = e.lock().expect("mutex");
    let antes = d.notas.len();
    d.notas.retain(|n| n.id != id);
    if d.notas.len() < antes { StatusCode::NO_CONTENT } else { StatusCode::NOT_FOUND }
}
