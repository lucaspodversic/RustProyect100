//! # Acortador con analíticas
//!
//! - `POST /api/enlaces {url, codigo?}` → crea
//! - `GET /{codigo}` → 302 + registra clic (día, referrer host, dispositivo, visitante anónimo)
//! - `GET /api/enlaces/{codigo}/analiticas?desde=AAAA-MM-DD&hasta=AAAA-MM-DD`
//!   → `{total, unicos, por_dia[], referrers[], dispositivos[]}`
//!
//! ## Diseño
//! - Tabla `clics` (una fila por visita) en vez de un contador: permite
//!   cualquier agregación posterior con `GROUP BY`. Índice `(enlace_id, dia)`.
//! - `por_dia` **rellena con ceros** los días sin clics del rango (útil para
//!   graficar).
//! - Los bots se registran pero se excluyen de `total`/`unicos`.
//! - Reloj inyectable (`Estado::reloj`) para tests deterministas.
//! - El registro del clic no debe frenar la redirección: si falla, se loguea
//!   y se redirige igual.

pub mod analitica;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::sync::Arc;

pub type Reloj = Arc<dyn Fn() -> i64 + Send + Sync>;

#[derive(Clone)]
pub struct Estado {
    pub db: SqlitePool,
    pub reloj: Reloj,
    pub sal: Arc<str>,
}

pub fn reloj_real() -> Reloj {
    Arc::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0))
}

pub async fn crear_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let o = SqliteConnectOptions::from_str(url)?.create_if_missing(true).foreign_keys(true);
    let p = SqlitePoolOptions::new().max_connections(if url.contains(":memory:") { 1 } else { 5 }).connect_with(o).await?;
    for q in [
        "CREATE TABLE IF NOT EXISTS enlaces (id INTEGER PRIMARY KEY AUTOINCREMENT, codigo TEXT NOT NULL UNIQUE, url TEXT NOT NULL, creado INTEGER NOT NULL)",
        "CREATE TABLE IF NOT EXISTS clics (id INTEGER PRIMARY KEY AUTOINCREMENT,
            enlace_id INTEGER NOT NULL REFERENCES enlaces(id) ON DELETE CASCADE,
            ts INTEGER NOT NULL, dia TEXT NOT NULL, referrer TEXT NOT NULL,
            dispositivo TEXT NOT NULL, visitante TEXT NOT NULL)",
        "CREATE INDEX IF NOT EXISTS ix_clics_enlace_dia ON clics(enlace_id, dia)",
    ] {
        sqlx::query(q).execute(&p).await?;
    }
    Ok(p)
}

pub fn app(e: Estado) -> Router {
    Router::new()
        .route("/api/enlaces", post(crear))
        .route("/api/enlaces/{codigo}/analiticas", get(analiticas))
        .route("/{codigo}", get(redirigir))
        .with_state(e)
}

fn error(c: StatusCode, m: &str) -> Response {
    (c, Json(serde_json::json!({"error": m}))).into_response()
}

const ALFABETO: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Código automático: hash de (url, ts, intento) en base62, 7 caracteres.
fn codigo_auto(url: &str, ts: i64, intento: u32) -> String {
    let h = blake3::hash(format!("{url}|{ts}|{intento}").as_bytes());
    h.as_bytes()[..7].iter().map(|b| ALFABETO[(*b as usize) % 62] as char).collect()
}

#[derive(Deserialize)]
struct NuevoEnlace {
    url: String,
    codigo: Option<String>,
}

async fn crear(State(e): State<Estado>, Json(n): Json<NuevoEnlace>) -> Response {
    let Ok(u) = url::Url::parse(&n.url) else { return error(StatusCode::UNPROCESSABLE_ENTITY, "url inválida") };
    if !matches!(u.scheme(), "http" | "https") {
        return error(StatusCode::UNPROCESSABLE_ENTITY, "solo http/https");
    }
    if let Some(c) = &n.codigo {
        if c == "api" || !(3..=32).contains(&c.len()) || !c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return error(StatusCode::UNPROCESSABLE_ENTITY, "código: 3-32 caracteres alfanuméricos o '-'");
        }
    }
    let ahora = (e.reloj)();
    // Con código automático reintentamos ante colisión (muy improbable).
    for intento in 0..5 {
        let codigo = n.codigo.clone().unwrap_or_else(|| codigo_auto(u.as_str(), ahora, intento));
        let r = sqlx::query("INSERT INTO enlaces (codigo, url, creado) VALUES (?, ?, ?)").bind(&codigo).bind(u.as_str()).bind(ahora).execute(&e.db).await;
        match r {
            Ok(_) => return (StatusCode::CREATED, Json(serde_json::json!({"codigo": codigo, "url": u.as_str()}))).into_response(),
            Err(sqlx::Error::Database(d)) if d.is_unique_violation() => {
                if n.codigo.is_some() {
                    return error(StatusCode::CONFLICT, "el código ya existe");
                }
            }
            Err(err) => {
                eprintln!("db: {err}");
                return error(StatusCode::INTERNAL_SERVER_ERROR, "error interno");
            }
        }
    }
    error(StatusCode::INTERNAL_SERVER_ERROR, "no se pudo generar un código único")
}

async fn redirigir(State(e): State<Estado>, Path(codigo): Path<String>, h: HeaderMap) -> Response {
    let fila: Option<(i64, String)> = match sqlx::query_as("SELECT id, url FROM enlaces WHERE codigo = ?").bind(&codigo).fetch_optional(&e.db).await {
        Ok(f) => f,
        Err(err) => {
            eprintln!("db: {err}");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "error interno");
        }
    };
    let Some((id, destino)) = fila else { return error(StatusCode::NOT_FOUND, "enlace inexistente") };

    let hdr = |n: header::HeaderName| h.get(n).and_then(|v| v.to_str().ok()).map(str::to_owned);
    let ua = hdr(header::USER_AGENT).unwrap_or_default();
    // Detrás de un proxy, la IP real viene en X-Forwarded-For (primer valor).
    let ip = h.get("x-forwarded-for").and_then(|v| v.to_str().ok()).and_then(|v| v.split(',').next()).unwrap_or("?").trim().to_string();
    let ts = (e.reloj)();
    let dia = analitica::dia_de(ts);
    let r = sqlx::query("INSERT INTO clics (enlace_id, ts, dia, referrer, dispositivo, visitante) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(id)
        .bind(ts)
        .bind(&dia)
        .bind(analitica::host_referrer(hdr(header::REFERER).as_deref()))
        .bind(analitica::dispositivo(&ua))
        .bind(analitica::visitante(&e.sal, &ip, &ua, &dia))
        .execute(&e.db)
        .await;
    if let Err(err) = r {
        eprintln!("no se registró el clic: {err}"); // la redirección sigue igual
    }
    // 302 (no 301): un 301 lo cachea el navegador y dejaríamos de contar clics.
    (StatusCode::FOUND, [(header::LOCATION, destino)]).into_response()
}

#[derive(Deserialize)]
struct Rango {
    desde: Option<String>,
    hasta: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
struct Conteo {
    clave: String,
    clics: i64,
}

async fn analiticas(State(e): State<Estado>, Path(codigo): Path<String>, Query(r): Query<Rango>) -> Response {
    match analiticas_inner(&e, &codigo, r).await {
        Ok(v) => v,
        Err(err) => {
            eprintln!("db: {err}");
            error(StatusCode::INTERNAL_SERVER_ERROR, "error interno")
        }
    }
}

fn es_fecha(s: &str) -> bool {
    s.len() == 10 && s.as_bytes()[4] == b'-' && s.as_bytes()[7] == b'-' && s.bytes().enumerate().all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
}

async fn analiticas_inner(e: &Estado, codigo: &str, r: Rango) -> Result<Response, sqlx::Error> {
    let Some((id,)): Option<(i64,)> = sqlx::query_as("SELECT id FROM enlaces WHERE codigo = ?").bind(codigo).fetch_optional(&e.db).await? else {
        return Ok(error(StatusCode::NOT_FOUND, "enlace inexistente"));
    };
    // Rango por defecto: últimos 7 días (incluido hoy).
    let hoy = (e.reloj)();
    let hasta = r.hasta.unwrap_or_else(|| analitica::dia_de(hoy));
    let desde = r.desde.unwrap_or_else(|| analitica::dia_de(hoy - 6 * 86_400));
    if !es_fecha(&desde) || !es_fecha(&hasta) || desde > hasta {
        return Ok(error(StatusCode::BAD_REQUEST, "rango inválido (AAAA-MM-DD, desde ≤ hasta)"));
    }
    // Filtro común: enlace, rango y sin bots. Las fechas ISO se comparan bien como texto.
    let filtro = "FROM clics WHERE enlace_id = ? AND dia BETWEEN ? AND ? AND dispositivo <> 'bot'";
    let (total, unicos): (i64, i64) = sqlx::query_as(&format!("SELECT COUNT(*), COUNT(DISTINCT visitante) {filtro}")).bind(id).bind(&desde).bind(&hasta).fetch_one(&e.db).await?;
    let agrupar = |col: &str, orden: &str| format!("SELECT {col} AS clave, COUNT(*) AS clics {filtro} GROUP BY {col} ORDER BY {orden} LIMIT 400");
    let dias: Vec<Conteo> = sqlx::query_as(&agrupar("dia", "dia")).bind(id).bind(&desde).bind(&hasta).fetch_all(&e.db).await?;
    let refs: Vec<Conteo> = sqlx::query_as(&agrupar("referrer", "clics DESC, clave").replace("LIMIT 400", "LIMIT 10")).bind(id).bind(&desde).bind(&hasta).fetch_all(&e.db).await?;
    let disp: Vec<Conteo> = sqlx::query_as(&agrupar("dispositivo", "clics DESC, clave")).bind(id).bind(&desde).bind(&hasta).fetch_all(&e.db).await?;
    let bots: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM clics WHERE enlace_id = ? AND dia BETWEEN ? AND ? AND dispositivo = 'bot'").bind(id).bind(&desde).bind(&hasta).fetch_one(&e.db).await?;

    // Relleno con ceros día por día (máx. 366 días para acotar la respuesta).
    let mut por_dia = vec![];
    let mut ts = fecha_a_ts(&desde);
    while por_dia.len() < 366 {
        let d = analitica::dia_de(ts);
        if d > hasta {
            break;
        }
        let n = dias.iter().find(|c| c.clave == d).map_or(0, |c| c.clics);
        por_dia.push(serde_json::json!({"dia": d, "clics": n}));
        ts += 86_400;
    }
    Ok(Json(serde_json::json!({
        "codigo": codigo, "desde": desde, "hasta": hasta,
        "total": total, "unicos": unicos, "bots": bots.0,
        "por_dia": por_dia, "referrers": refs, "dispositivos": disp,
    }))
    .into_response())
}

/// "AAAA-MM-DD" → timestamp del mediodía UTC (inverso de `dia_de`).
fn fecha_a_ts(f: &str) -> i64 {
    let y: i64 = f[..4].parse().unwrap_or(1970);
    let m: i64 = f[5..7].parse().unwrap_or(1);
    let d: i64 = f[8..].parse().unwrap_or(1);
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) * 86_400 + 43_200
}

#[cfg(test)]
mod tests {
    #[test]
    fn ida_y_vuelta_de_fechas() {
        for f in ["1970-01-01", "2024-02-29", "2026-12-31", "2000-03-01"] {
            assert_eq!(super::analitica::dia_de(super::fecha_a_ts(f)), f);
        }
    }
}
