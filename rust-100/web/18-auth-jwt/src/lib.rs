//! # Autenticación con JWT
//!
//! Flujo:
//! 1. `POST /auth/registro` `{email, clave}` → guarda el usuario con la clave
//!    hasheada con **Argon2id** (sal aleatoria por usuario, nunca texto plano).
//! 2. `POST /auth/login` → verifica y devuelve `{"token": "<JWT>", "expira_en": s}`.
//! 3. Rutas protegidas leen `Authorization: Bearer <JWT>` mediante el extractor
//!    [`Usuario`] (implementa `FromRequestParts`): si el token falta, está mal
//!    firmado o expiró, el handler ni se ejecuta → 401.
//! 4. Rol `admin` para rutas administrativas → 403 si no corresponde.
//!
//! El JWT (HS256) contiene `sub` (id), `email`, `rol`, `iat`, `exp`.
//! Como es *stateless*, el servidor no guarda sesiones: la validez la da la firma.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use axum::extract::{FromRef, FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Configuración de seguridad.
pub struct Config {
    /// Secreto HMAC del JWT (mínimo 32 bytes en producción).
    pub secreto: Vec<u8>,
    /// Duración del token en segundos.
    pub duracion: u64,
    /// Parámetros Argon2 (memoria KiB, iteraciones, paralelismo). Los tests usan
    /// valores bajos para ser rápidos; producción usa los recomendados por OWASP.
    pub argon: Params,
}

impl Config {
    pub fn produccion(secreto: Vec<u8>) -> Self {
        Config { secreto, duracion: 3600, argon: Params::new(19 * 1024, 2, 1, None).expect("params") }
    }
    fn argon(&self) -> Argon2<'static> {
        Argon2::new(Algorithm::Argon2id, Version::V0x13, self.argon.clone())
    }
}

/// Estado compartido.
#[derive(Clone)]
pub struct Estado {
    pub db: SqlitePool,
    pub cfg: Arc<Config>,
}

/// Contenido (claims) del JWT.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Claims {
    pub sub: i64,
    pub email: String,
    pub rol: String,
    pub iat: u64,
    pub exp: u64,
}

/// Error HTTP con mensaje JSON.
#[derive(Debug)]
pub struct ErrorHttp(pub StatusCode, pub String);
impl IntoResponse for ErrorHttp {
    fn into_response(self) -> Response {
        let mut r = (self.0, Json(serde_json::json!({"error": self.1}))).into_response();
        if self.0 == StatusCode::UNAUTHORIZED {
            // RFC 6750: indicar el esquema esperado.
            r.headers_mut().insert(header::WWW_AUTHENTICATE, "Bearer".parse().expect("valor"));
        }
        r
    }
}
fn err(c: StatusCode, m: &str) -> ErrorHttp {
    ErrorHttp(c, m.to_string())
}
impl From<sqlx::Error> for ErrorHttp {
    fn from(e: sqlx::Error) -> Self {
        eprintln!("db: {e}");
        err(StatusCode::INTERNAL_SERVER_ERROR, "error interno")
    }
}

fn ahora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("reloj").as_secs()
}

/// Emite un JWT firmado.
pub fn emitir(cfg: &Config, id: i64, email: &str, rol: &str) -> String {
    let iat = ahora();
    let c = Claims { sub: id, email: email.into(), rol: rol.into(), iat, exp: iat + cfg.duracion };
    encode(&Header::default(), &c, &EncodingKey::from_secret(&cfg.secreto)).expect("firmar")
}

/// Verifica firma y expiración (sin margen de tolerancia).
pub fn verificar(cfg: &Config, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut v = Validation::default(); // HS256, valida exp
    v.leeway = 0;
    decode::<Claims>(token, &DecodingKey::from_secret(&cfg.secreto), &v).map(|d| d.claims)
}

/// Usuario autenticado: extractor que valida el header Authorization.
#[derive(Debug, Clone)]
pub struct Usuario(pub Claims);

impl<S> FromRequestParts<S> for Usuario
where
    Estado: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ErrorHttp;

    async fn from_request_parts(parts: &mut Parts, estado: &S) -> Result<Self, Self::Rejection> {
        let e = Estado::from_ref(estado);
        let h = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "falta Authorization"))?;
        let token = h.strip_prefix("Bearer ").ok_or_else(|| err(StatusCode::UNAUTHORIZED, "se espera esquema Bearer"))?;
        let c = verificar(&e.cfg, token).map_err(|x| {
            let m = match x.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => "token expirado",
                _ => "token inválido",
            };
            err(StatusCode::UNAUTHORIZED, m)
        })?;
        Ok(Usuario(c))
    }
}

/// Variante que además exige rol admin.
pub struct Admin(pub Claims);
impl<S> FromRequestParts<S> for Admin
where
    Estado: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ErrorHttp;
    async fn from_request_parts(parts: &mut Parts, estado: &S) -> Result<Self, Self::Rejection> {
        let Usuario(c) = Usuario::from_request_parts(parts, estado).await?;
        if c.rol != "admin" {
            return Err(err(StatusCode::FORBIDDEN, "requiere rol admin"));
        }
        Ok(Admin(c))
    }
}

#[derive(Deserialize)]
struct Credenciales {
    email: String,
    clave: String,
}

/// Esquema + pool.
pub async fn crear_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let o = SqliteConnectOptions::from_str(url)?.create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(if url.contains(":memory:") { 1 } else { 5 }).connect_with(o).await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS usuarios (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL UNIQUE COLLATE NOCASE,
            hash TEXT NOT NULL,
            rol TEXT NOT NULL DEFAULT 'usuario'
        )",
    )
    .execute(&pool)
    .await?;
    Ok(pool)
}

pub fn app(e: Estado) -> Router {
    Router::new()
        .route("/auth/registro", post(registro))
        .route("/auth/login", post(login))
        .route("/yo", get(yo))
        .route("/admin/usuarios", get(listar_usuarios))
        .with_state(e)
}

fn validar(c: &Credenciales) -> Result<(), ErrorHttp> {
    let e = c.email.trim();
    // Validación mínima: algo@algo.algo
    let ok = e.split_once('@').is_some_and(|(u, d)| !u.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.'));
    if !ok {
        return Err(err(StatusCode::UNPROCESSABLE_ENTITY, "email inválido"));
    }
    if c.clave.chars().count() < 8 {
        return Err(err(StatusCode::UNPROCESSABLE_ENTITY, "la clave debe tener al menos 8 caracteres"));
    }
    Ok(())
}

async fn registro(State(e): State<Estado>, Json(c): Json<Credenciales>) -> Result<Response, ErrorHttp> {
    validar(&c)?;
    let sal = SaltString::generate(&mut OsRng);
    let hash = e.cfg.argon().hash_password(c.clave.as_bytes(), &sal).map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "hash"))?.to_string();
    // El primer usuario registrado es admin (útil para la demo).
    let r = sqlx::query_as::<_, (i64, String)>(
        "INSERT INTO usuarios (email, hash, rol)
         VALUES (?, ?, CASE WHEN (SELECT COUNT(*) FROM usuarios) = 0 THEN 'admin' ELSE 'usuario' END)
         RETURNING id, rol",
    )
    .bind(c.email.trim())
    .bind(hash)
    .fetch_one(&e.db)
    .await;
    match r {
        Ok((id, rol)) => Ok((StatusCode::CREATED, Json(serde_json::json!({"id": id, "email": c.email.trim(), "rol": rol}))).into_response()),
        Err(sqlx::Error::Database(d)) if d.is_unique_violation() => Err(err(StatusCode::CONFLICT, "email ya registrado")),
        Err(x) => Err(x.into()),
    }
}

async fn login(State(e): State<Estado>, Json(c): Json<Credenciales>) -> Result<Json<serde_json::Value>, ErrorHttp> {
    let fila = sqlx::query_as::<_, (i64, String, String, String)>("SELECT id, email, hash, rol FROM usuarios WHERE email = ?")
        .bind(c.email.trim())
        .fetch_optional(&e.db)
        .await?;
    // Mismo mensaje para "no existe" y "clave mal": no revelar qué emails existen.
    let invalido = || err(StatusCode::UNAUTHORIZED, "credenciales inválidas");
    let (id, email, hash, rol) = fila.ok_or_else(invalido)?;
    let ph = PasswordHash::new(&hash).map_err(|_| invalido())?;
    e.cfg.argon().verify_password(c.clave.as_bytes(), &ph).map_err(|_| invalido())?;
    Ok(Json(serde_json::json!({"token": emitir(&e.cfg, id, &email, &rol), "tipo": "Bearer", "expira_en": e.cfg.duracion})))
}

async fn yo(Usuario(c): Usuario) -> Json<Claims> {
    Json(c)
}

async fn listar_usuarios(Admin(_): Admin, State(e): State<Estado>) -> Result<Json<Vec<serde_json::Value>>, ErrorHttp> {
    let v = sqlx::query_as::<_, (i64, String, String)>("SELECT id, email, rol FROM usuarios ORDER BY id").fetch_all(&e.db).await?;
    Ok(Json(v.into_iter().map(|(id, email, rol)| serde_json::json!({"id": id, "email": email, "rol": rol})).collect()))
}
