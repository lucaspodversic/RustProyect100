//! # API de reservas
//!
//! | Método | Ruta | Descripción |
//! |---|---|---|
//! | POST | `/recursos` | crear recurso (sala, cancha…) con horario y granularidad |
//! | GET | `/recursos` | listar |
//! | GET | `/recursos/{id}/disponibilidad?fecha=YYYY-MM-DD&duracion=60` | huecos libres e inicios posibles |
//! | POST | `/reservas` | reservar → 201, o **409** con las reservas en conflicto |
//! | GET | `/reservas?recurso_id=&fecha=` | listar reservas activas |
//! | DELETE | `/reservas/{id}` | cancelar (borrado lógico) |
//!
//! ## Conflictos sin carreras
//! La comprobación y la inserción son **una sola sentencia**
//! `INSERT … SELECT … WHERE NOT EXISTS (solapamiento)`. SQLite ejecuta cada
//! sentencia de forma atómica, así que dos pedidos simultáneos para el mismo
//! horario no pueden ganar los dos (un "check, luego insert" en dos pasos sí
//! tendría esa carrera).

pub mod agenda;

use agenda::{formatear, huecos_libres, inicios_posibles, parse_fecha, parse_fecha_hora, parse_hora, MIN_DIA};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::sync::Arc;

/// Duración máxima de una reserva (minutos).
pub const MAX_DURACION: i64 = 8 * 60;

#[derive(Clone)]
pub struct Estado {
    pub pool: SqlitePool,
    /// "Ahora" en minutos absolutos (inyectable para tests).
    pub ahora: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl Estado {
    pub fn nuevo(pool: SqlitePool) -> Self {
        Self {
            pool,
            ahora: Arc::new(|| {
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 / 60
            }),
        }
    }
}

pub async fn crear_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(url)?.create_if_missing(true).foreign_keys(true);
    let max = if url.contains(":memory:") { 1 } else { 5 };
    let pool = SqlitePoolOptions::new().max_connections(max).connect_with(opts).await?;
    for sql in [
        "CREATE TABLE IF NOT EXISTS recursos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nombre TEXT NOT NULL,
            apertura INTEGER NOT NULL,      -- minutos desde medianoche
            cierre INTEGER NOT NULL,
            granularidad INTEGER NOT NULL)",
        "CREATE TABLE IF NOT EXISTS reservas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            recurso_id INTEGER NOT NULL REFERENCES recursos(id),
            inicio INTEGER NOT NULL,        -- minutos absolutos
            fin INTEGER NOT NULL,
            titular TEXT NOT NULL,
            activa INTEGER NOT NULL DEFAULT 1,
            CHECK (fin > inicio))",
        // Índice para la consulta de solapamiento por recurso.
        "CREATE INDEX IF NOT EXISTS idx_reservas_recurso ON reservas (recurso_id, activa, inicio)",
    ] {
        sqlx::query(sql).execute(&pool).await?;
    }
    Ok(pool)
}

// ---------------------------------------------------------------- errores

#[derive(Debug)]
pub enum ErrorApi {
    NoEncontrado,
    Validacion(String),
    Conflicto(Vec<ReservaOut>),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for ErrorApi {
    fn from(e: sqlx::Error) -> Self {
        ErrorApi::Db(e)
    }
}

impl IntoResponse for ErrorApi {
    fn into_response(self) -> Response {
        match self {
            ErrorApi::NoEncontrado => (StatusCode::NOT_FOUND, Json(json!({"error": "no encontrado"}))).into_response(),
            ErrorApi::Validacion(m) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error": m}))).into_response(),
            ErrorApi::Conflicto(c) => (
                StatusCode::CONFLICT,
                Json(json!({"error": "el horario se superpone con otras reservas", "conflictos": c})),
            ).into_response(),
            ErrorApi::Db(e) => {
                eprintln!("error de base: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "error interno"}))).into_response()
            }
        }
    }
}

fn invalido<T>(m: &str) -> Result<T, ErrorApi> {
    Err(ErrorApi::Validacion(m.into()))
}

// ---------------------------------------------------------------- modelos

#[derive(Deserialize)]
pub struct NuevoRecurso {
    nombre: String,
    apertura: String,
    cierre: String,
    #[serde(default = "quince")]
    granularidad_min: i64,
}
fn quince() -> i64 {
    15
}

#[derive(Serialize, sqlx::FromRow, Clone, Copy)]
struct FilaRecurso {
    id: i64,
    apertura: i64,
    cierre: i64,
    granularidad: i64,
}

#[derive(Debug, Serialize)]
pub struct ReservaOut {
    pub id: i64,
    pub recurso_id: i64,
    pub inicio: String,
    pub fin: String,
    pub titular: String,
}

#[derive(sqlx::FromRow)]
struct FilaReserva {
    id: i64,
    recurso_id: i64,
    inicio: i64,
    fin: i64,
    titular: String,
}

impl From<FilaReserva> for ReservaOut {
    fn from(f: FilaReserva) -> Self {
        ReservaOut { id: f.id, recurso_id: f.recurso_id, inicio: formatear(f.inicio), fin: formatear(f.fin), titular: f.titular }
    }
}

// ---------------------------------------------------------------- router

pub fn app(e: Estado) -> Router {
    Router::new()
        .route("/recursos", post(crear_recurso).get(listar_recursos))
        .route("/recursos/{id}/disponibilidad", get(disponibilidad))
        .route("/reservas", post(reservar).get(listar_reservas))
        .route("/reservas/{id}", delete(cancelar))
        .with_state(e)
}

async fn crear_recurso(State(e): State<Estado>, Json(r): Json<NuevoRecurso>) -> Result<Response, ErrorApi> {
    let nombre = r.nombre.trim();
    if nombre.is_empty() || nombre.chars().count() > 80 {
        return invalido("nombre: 1..=80 caracteres");
    }
    let (Some(a), Some(c)) = (parse_hora(&r.apertura), parse_hora(&r.cierre)) else {
        return invalido("apertura/cierre deben ser HH:MM");
    };
    if a >= c {
        return invalido("la apertura debe ser anterior al cierre");
    }
    let g = r.granularidad_min;
    // La granularidad debe dividir al día para que la alineación sea consistente.
    if !(5..=240).contains(&g) || MIN_DIA % g != 0 || a % g != 0 || c % g != 0 {
        return invalido("granularidad_min: divisor de 1440 entre 5 y 240, y apertura/cierre alineados");
    }
    let id = sqlx::query("INSERT INTO recursos (nombre, apertura, cierre, granularidad) VALUES (?, ?, ?, ?)")
        .bind(nombre).bind(a).bind(c).bind(g).execute(&e.pool).await?.last_insert_rowid();
    Ok((StatusCode::CREATED, Json(json!({"id": id, "nombre": nombre, "apertura": r.apertura, "cierre": r.cierre, "granularidad_min": g}))).into_response())
}

async fn listar_recursos(State(e): State<Estado>) -> Result<Json<serde_json::Value>, ErrorApi> {
    let filas: Vec<(i64, String, i64, i64, i64)> =
        sqlx::query_as("SELECT id, nombre, apertura, cierre, granularidad FROM recursos ORDER BY id").fetch_all(&e.pool).await?;
    let hhmm = |m: i64| format!("{:02}:{:02}", m / 60, m % 60);
    Ok(Json(json!(filas.into_iter().map(|(id, n, a, c, g)| json!({
        "id": id, "nombre": n, "apertura": hhmm(a), "cierre": hhmm(c), "granularidad_min": g
    })).collect::<Vec<_>>())))
}

async fn recurso(e: &Estado, id: i64) -> Result<FilaRecurso, ErrorApi> {
    sqlx::query_as("SELECT id, apertura, cierre, granularidad FROM recursos WHERE id = ?")
        .bind(id).fetch_optional(&e.pool).await?.ok_or(ErrorApi::NoEncontrado)
}

async fn reservas_en(e: &Estado, recurso_id: i64, desde: i64, hasta: i64) -> Result<Vec<FilaReserva>, ErrorApi> {
    // Mismo predicado de solapamiento que agenda::se_solapan, en SQL.
    Ok(sqlx::query_as(
        "SELECT id, recurso_id, inicio, fin, titular FROM reservas
         WHERE recurso_id = ? AND activa = 1 AND inicio < ? AND fin > ? ORDER BY inicio",
    )
    .bind(recurso_id).bind(hasta).bind(desde).fetch_all(&e.pool).await?)
}

#[derive(Deserialize)]
pub struct ConsultaDisp {
    fecha: String,
    #[serde(default)]
    duracion: Option<i64>,
}

async fn disponibilidad(State(e): State<Estado>, Path(id): Path<i64>, Query(q): Query<ConsultaDisp>) -> Result<Json<serde_json::Value>, ErrorApi> {
    let r = recurso(&e, id).await?;
    let Some(dia) = parse_fecha(&q.fecha) else { return invalido("fecha debe ser YYYY-MM-DD") };
    let dur = q.duracion.unwrap_or(r.granularidad);
    if dur <= 0 || dur > MAX_DURACION || dur % r.granularidad != 0 {
        return invalido("duracion debe ser múltiplo de la granularidad y ≤ 480");
    }
    // Ventana del día; si es hoy, no ofrecemos horarios ya pasados.
    let desde = (dia + r.apertura).max((e.ahora)());
    let hasta = dia + r.cierre;
    let ocupadas = reservas_en(&e, id, dia + r.apertura, hasta).await?;
    let occ: Vec<(i64, i64)> = ocupadas.iter().map(|f| (f.inicio, f.fin)).collect();
    let libres = if desde < hasta { huecos_libres(desde, hasta, &occ) } else { vec![] };
    let inicios = inicios_posibles(&libres, dur, r.granularidad);
    Ok(Json(json!({
        "recurso_id": id,
        "fecha": q.fecha,
        "duracion_min": dur,
        "libres": libres.iter().map(|&(a, b)| json!({"inicio": formatear(a), "fin": formatear(b)})).collect::<Vec<_>>(),
        "ocupados": occ.iter().map(|&(a, b)| json!({"inicio": formatear(a), "fin": formatear(b)})).collect::<Vec<_>>(),
        "inicios_posibles": inicios.into_iter().map(formatear).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct NuevaReserva {
    recurso_id: i64,
    inicio: String,
    fin: String,
    titular: String,
}

async fn reservar(State(e): State<Estado>, Json(n): Json<NuevaReserva>) -> Result<Response, ErrorApi> {
    let r = recurso(&e, n.recurso_id).await?;
    let titular = n.titular.trim();
    if titular.is_empty() || titular.chars().count() > 80 {
        return invalido("titular: 1..=80 caracteres");
    }
    let (Some(ini), Some(fin)) = (parse_fecha_hora(&n.inicio), parse_fecha_hora(&n.fin)) else {
        return invalido("inicio/fin deben ser YYYY-MM-DDTHH:MM");
    };
    if fin <= ini {
        return invalido("fin debe ser posterior a inicio");
    }
    if fin - ini > MAX_DURACION {
        return invalido("una reserva no puede durar más de 8 horas");
    }
    if ini < (e.ahora)() {
        return invalido("no se puede reservar en el pasado");
    }
    let dia = ini.div_euclid(MIN_DIA) * MIN_DIA;
    if ini < dia + r.apertura || fin > dia + r.cierre {
        return invalido("fuera del horario del recurso");
    }
    if ini % r.granularidad != 0 || fin % r.granularidad != 0 {
        return invalido(&format!("los horarios deben estar alineados a {} minutos", r.granularidad));
    }
    // Comprobación + inserción ATÓMICA (ver doc del módulo).
    let id: Option<i64> = sqlx::query_scalar(
        "INSERT INTO reservas (recurso_id, inicio, fin, titular)
         SELECT ?1, ?2, ?3, ?4
         WHERE NOT EXISTS (SELECT 1 FROM reservas
                           WHERE recurso_id = ?1 AND activa = 1 AND inicio < ?3 AND fin > ?2)
         RETURNING id",
    )
    .bind(n.recurso_id).bind(ini).bind(fin).bind(titular)
    .fetch_optional(&e.pool).await?;
    match id {
        Some(id) => Ok((StatusCode::CREATED, Json(ReservaOut {
            id, recurso_id: n.recurso_id, inicio: formatear(ini), fin: formatear(fin), titular: titular.into(),
        })).into_response()),
        None => {
            let c = reservas_en(&e, n.recurso_id, ini, fin).await?;
            Err(ErrorApi::Conflicto(c.into_iter().map(Into::into).collect()))
        }
    }
}

#[derive(Deserialize)]
pub struct FiltroReservas {
    recurso_id: Option<i64>,
    fecha: Option<String>,
}

async fn listar_reservas(State(e): State<Estado>, Query(f): Query<FiltroReservas>) -> Result<Json<Vec<ReservaOut>>, ErrorApi> {
    let (desde, hasta) = match f.fecha.as_deref() {
        None => (i64::MIN, i64::MAX),
        Some(s) => match parse_fecha(s) {
            Some(d) => (d, d + MIN_DIA),
            None => return invalido("fecha debe ser YYYY-MM-DD"),
        },
    };
    let filas: Vec<FilaReserva> = sqlx::query_as(
        "SELECT id, recurso_id, inicio, fin, titular FROM reservas
         WHERE activa = 1 AND (?1 IS NULL OR recurso_id = ?1) AND inicio < ?3 AND fin > ?2
         ORDER BY inicio",
    )
    .bind(f.recurso_id).bind(desde).bind(hasta).fetch_all(&e.pool).await?;
    Ok(Json(filas.into_iter().map(Into::into).collect()))
}

async fn cancelar(State(e): State<Estado>, Path(id): Path<i64>) -> Result<StatusCode, ErrorApi> {
    // Borrado lógico: se conserva el historial; sólo se cancelan reservas activas.
    let n = sqlx::query("UPDATE reservas SET activa = 0 WHERE id = ? AND activa = 1")
        .bind(id).execute(&e.pool).await?.rows_affected();
    if n == 0 { Err(ErrorApi::NoEncontrado) } else { Ok(StatusCode::NO_CONTENT) }
}
