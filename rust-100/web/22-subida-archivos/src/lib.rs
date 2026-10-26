//! # Subida y descarga de archivos
//!
//! - `POST /archivos` (multipart, campo `archivo`) → `201 {id, nombre, mime, tamano}`
//! - `GET /archivos` → listado de metadatos
//! - `GET /archivos/{id}` → descarga (`Content-Disposition: attachment`)
//! - `DELETE /archivos/{id}`
//!
//! ## Defensas
//! 1. **Tamaño**: se lee el campo *en trozos* (`field.chunk()`) y se corta
//!    apenas supera el máximo (413), sin cargar todo en memoria. Además,
//!    `DefaultBodyLimit` limita el cuerpo HTTP completo.
//! 2. **MIME**: detectado por contenido ([`mime::detectar`]) y comparado con
//!    una lista blanca → 415 si no está.
//! 3. **Nombres**: el nombre del cliente solo se guarda como metadato
//!    saneado; en disco el archivo se llama por su **hash BLAKE3**
//!    (almacenamiento direccionado por contenido → deduplicación gratis y
//!    cero riesgo de path traversal).
//! 4. **Descarga**: `X-Content-Type-Options: nosniff` y `attachment` para que
//!    el navegador no ejecute HTML/JS subido.
//! 5. El `id` del path se valida (64 hex) antes de tocar el disco.

pub mod mime;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct Config {
    pub dir: PathBuf,
    pub max_bytes: usize,
    pub permitidos: Vec<&'static str>,
}

impl Config {
    pub fn new(dir: PathBuf) -> Self {
        Config { dir, max_bytes: 5 * 1024 * 1024, permitidos: vec!["image/png", "image/jpeg", "image/gif", "image/webp", "application/pdf", "text/plain"] }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Meta {
    pub id: String,
    pub nombre: String,
    pub mime: String,
    pub tamano: usize,
}

pub enum ErrorApi {
    Status(StatusCode, String),
    Io(std::io::Error),
}
impl From<std::io::Error> for ErrorApi {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::NotFound {
            ErrorApi::Status(StatusCode::NOT_FOUND, "no encontrado".into())
        } else {
            ErrorApi::Io(e)
        }
    }
}
impl IntoResponse for ErrorApi {
    fn into_response(self) -> Response {
        let (c, m) = match self {
            ErrorApi::Status(c, m) => (c, m),
            ErrorApi::Io(e) => {
                eprintln!("io: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "error interno".into())
            }
        };
        (c, Json(serde_json::json!({"error": m}))).into_response()
    }
}
fn err(c: StatusCode, m: impl Into<String>) -> ErrorApi {
    ErrorApi::Status(c, m.into())
}

/// Sanea un nombre de archivo: solo el último componente, sin controles ni
/// comillas (va dentro de un header), y longitud acotada.
pub fn sanear_nombre(n: &str) -> String {
    let base = n.rsplit(['/', '\\']).next().unwrap_or("");
    let limpio: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '"' | ';'))
        .take(100)
        .collect();
    let limpio = limpio.trim().trim_start_matches('.').to_string();
    if limpio.is_empty() { "archivo".into() } else { limpio }
}

fn id_valido(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

pub fn app(cfg: Config) -> Router {
    // Límite del cuerpo completo: archivo + margen para cabeceras multipart.
    let limite = cfg.max_bytes + 16 * 1024;
    Router::new()
        .route("/archivos", get(listar).post(subir))
        .route("/archivos/{id}", get(descargar).delete(borrar))
        .layer(DefaultBodyLimit::max(limite))
        .with_state(Arc::new(cfg))
}

/// Traduce errores de multipart. Si saltó `DefaultBodyLimit`, axum lo informa
/// como error de multipart; lo detectamos por estado o por mensaje.
fn error_multipart(e: axum::extract::multipart::MultipartError) -> ErrorApi {
    let texto = e.body_text();
    eprintln!("multipart: {texto} ({})", e.status());
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE || texto.to_lowercase().contains("limit") {
        err(StatusCode::PAYLOAD_TOO_LARGE, "archivo demasiado grande")
    } else {
        err(StatusCode::BAD_REQUEST, texto)
    }
}

async fn subir(State(cfg): State<Arc<Config>>, mut mp: Multipart) -> Result<(StatusCode, Json<Meta>), ErrorApi> {
    while let Some(mut campo) = mp.next_field().await.map_err(error_multipart)? {
        if campo.name() != Some("archivo") {
            continue;
        }
        let nombre = sanear_nombre(campo.file_name().unwrap_or("archivo"));
        let mut datos = Vec::new();
        // Lectura incremental: cortamos en cuanto se pasa del límite.
        while let Some(trozo) = campo.chunk().await.map_err(error_multipart)? {
            if datos.len() + trozo.len() > cfg.max_bytes {
                return Err(err(StatusCode::PAYLOAD_TOO_LARGE, format!("máximo {} bytes", cfg.max_bytes)));
            }
            datos.extend_from_slice(&trozo);
        }
        if datos.is_empty() {
            return Err(err(StatusCode::BAD_REQUEST, "archivo vacío"));
        }
        let mime = mime::detectar(&datos).filter(|m| cfg.permitidos.contains(m)).ok_or_else(|| err(StatusCode::UNSUPPORTED_MEDIA_TYPE, "tipo de archivo no permitido"))?;
        let id = blake3::hash(&datos).to_hex().to_string();
        let meta = Meta { id: id.clone(), nombre, mime: mime.into(), tamano: datos.len() };
        tokio::fs::create_dir_all(&cfg.dir).await?;
        // Escritura atómica: temporal + rename (nadie ve un archivo a medias).
        let tmp = cfg.dir.join(format!("{id}.tmp"));
        tokio::fs::write(&tmp, &datos).await?;
        tokio::fs::rename(&tmp, cfg.dir.join(&id)).await?;
        tokio::fs::write(cfg.dir.join(format!("{id}.json")), serde_json::to_vec(&meta).expect("json")).await?;
        return Ok((StatusCode::CREATED, Json(meta)));
    }
    Err(err(StatusCode::BAD_REQUEST, "falta el campo 'archivo'"))
}

async fn leer_meta(cfg: &Config, id: &str) -> Result<Meta, ErrorApi> {
    if !id_valido(id) {
        return Err(err(StatusCode::NOT_FOUND, "no encontrado"));
    }
    let b = tokio::fs::read(cfg.dir.join(format!("{id}.json"))).await?;
    serde_json::from_slice(&b).map_err(|e| ErrorApi::Io(std::io::Error::other(e)))
}

async fn listar(State(cfg): State<Arc<Config>>) -> Result<Json<Vec<Meta>>, ErrorApi> {
    let mut v = vec![];
    let Ok(mut rd) = tokio::fs::read_dir(&cfg.dir).await else { return Ok(Json(v)) };
    while let Some(e) = rd.next_entry().await? {
        let n = e.file_name().to_string_lossy().to_string();
        if let Some(id) = n.strip_suffix(".json") {
            if let Ok(m) = leer_meta(&cfg, id).await {
                v.push(m);
            }
        }
    }
    v.sort_by(|a, b| a.nombre.cmp(&b.nombre));
    Ok(Json(v))
}

async fn descargar(State(cfg): State<Arc<Config>>, Path(id): Path<String>) -> Result<Response, ErrorApi> {
    let m = leer_meta(&cfg, &id).await?;
    // Streaming desde disco: no se carga el archivo entero en memoria.
    let f = tokio::fs::File::open(cfg.dir.join(&id)).await?;
    let cuerpo = Body::from_stream(tokio_util::io::ReaderStream::with_capacity(f, 64 * 1024));
    Ok((
        [
            (header::CONTENT_TYPE, m.mime.clone()),
            (header::CONTENT_LENGTH, m.tamano.to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", m.nombre)),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
        ],
        cuerpo,
    )
        .into_response())
}

async fn borrar(State(cfg): State<Arc<Config>>, Path(id): Path<String>) -> Result<StatusCode, ErrorApi> {
    leer_meta(&cfg, &id).await?;
    tokio::fs::remove_file(cfg.dir.join(&id)).await?;
    tokio::fs::remove_file(cfg.dir.join(format!("{id}.json"))).await?;
    Ok(StatusCode::NO_CONTENT)
}
