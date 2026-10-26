//! # Portal de administración
//!
//! Aplicación web clásica (HTML renderizado en el servidor, sin JS) con:
//!
//! - **Login** con contraseñas Argon2id.
//! - **Sesiones del lado del servidor**: el navegador sólo guarda un token
//!   aleatorio de 256 bits en una cookie `HttpOnly; SameSite=Strict`; en la
//!   base guardamos su **hash SHA-256** (si se filtra la base, no se pueden
//!   reutilizar las sesiones). Expiran por TTL y se revocan al cerrar sesión
//!   o al desactivar el usuario.
//! - **CSRF**: cada sesión tiene un token propio que va oculto en cada
//!   formulario y se verifica en cada POST (defensa adicional a SameSite).
//! - **Roles**: `admin` (gestiona usuarios), `editor` y `lector` (sólo ven).
//! - **Panel** con métricas, tabla de usuarios y registro de **auditoría**.
//! - Todo texto de usuario se escapa con [`esc`] antes de ir al HTML (anti-XSS).
//!
//! | Método | Ruta | Quién |
//! |---|---|---|
//! | GET/POST | `/login` | público |
//! | POST | `/logout` | sesión + CSRF |
//! | GET | `/panel` | sesión |
//! | GET | `/api/yo` | sesión (JSON) |
//! | POST | `/panel/usuarios` | admin + CSRF |
//! | POST | `/panel/usuarios/{id}/rol` | admin + CSRF |
//! | POST | `/panel/usuarios/{id}/activo` | admin + CSRF |

use argon2::password_hash::rand_core::{OsRng, RngCore};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use axum::extract::{FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::sync::Arc;

pub const COOKIE: &str = "sid";
pub const ROLES: [&str; 3] = ["admin", "editor", "lector"];

/// Reloj inyectable (segundos Unix) para testear expiraciones.
pub type Reloj = Arc<dyn Fn() -> i64 + Send + Sync>;

/// Estado compartido de la aplicación.
#[derive(Clone)]
pub struct Estado {
    pub pool: SqlitePool,
    pub argon: Params,
    pub reloj: Reloj,
    /// Vida de una sesión en segundos.
    pub ttl: i64,
    /// Si es `true` la cookie lleva `Secure` (sólo HTTPS). En dev local, `false`.
    pub cookie_segura: bool,
}

impl Estado {
    pub fn nuevo(pool: SqlitePool) -> Self {
        Self {
            pool,
            argon: Params::default(),
            reloj: Arc::new(|| {
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
            }),
            ttl: 8 * 3600,
            cookie_segura: false,
        }
    }
    fn argon(&self) -> Argon2<'static> {
        Argon2::new(Algorithm::Argon2id, Version::V0x13, self.argon.clone())
    }
    fn ahora(&self) -> i64 {
        (self.reloj)()
    }
}

pub async fn crear_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(url)?.create_if_missing(true).foreign_keys(true);
    // `:memory:` es por conexión → una sola conexión para compartir la base.
    let max = if url.contains(":memory:") { 1 } else { 5 };
    let pool = SqlitePoolOptions::new().max_connections(max).connect_with(opts).await?;
    for sql in [
        "CREATE TABLE IF NOT EXISTS usuarios (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nombre TEXT NOT NULL UNIQUE COLLATE NOCASE,
            hash TEXT NOT NULL,
            rol TEXT NOT NULL CHECK (rol IN ('admin','editor','lector')),
            activo INTEGER NOT NULL DEFAULT 1,
            ultimo_login INTEGER)",
        "CREATE TABLE IF NOT EXISTS sesiones (
            token_hash TEXT PRIMARY KEY,
            usuario_id INTEGER NOT NULL REFERENCES usuarios(id) ON DELETE CASCADE,
            csrf TEXT NOT NULL,
            expira INTEGER NOT NULL)",
        "CREATE TABLE IF NOT EXISTS auditoria (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL, actor TEXT NOT NULL, accion TEXT NOT NULL)",
    ] {
        sqlx::query(sql).execute(&pool).await?;
    }
    Ok(pool)
}

// ---------------------------------------------------------------- utilidades

/// Escapa los 5 caracteres peligrosos de HTML.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

/// 32 bytes aleatorios del SO en hex.
fn token_aleatorio() -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

/// Comparación en tiempo constante (evita filtrar el token por tiempos).
fn iguales_ct(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Lee una cookie del header `Cookie: a=1; b=2`.
pub fn leer_cookie(h: &HeaderMap, nombre: &str) -> Option<String> {
    h.get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|par| par.trim().split_once('='))
        .find(|(k, _)| *k == nombre)
        .map(|(_, v)| v.to_string())
}

pub fn nombre_valido(n: &str) -> bool {
    (3..=32).contains(&n.len()) && n.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

async fn auditar(pool: &SqlitePool, ts: i64, actor: &str, accion: &str) {
    // La auditoría nunca debe tumbar la operación principal: ignoramos el error.
    let _ = sqlx::query("INSERT INTO auditoria (ts, actor, accion) VALUES (?, ?, ?)")
        .bind(ts).bind(actor).bind(accion).execute(pool).await;
}

/// Crea un usuario (lo usa el panel y `main` para el admin inicial).
pub async fn crear_usuario(e: &Estado, nombre: &str, clave: &str, rol: &str) -> Result<i64, String> {
    if !nombre_valido(nombre) {
        return Err("nombre: 3..=32 caracteres [a-zA-Z0-9._-]".into());
    }
    if clave.chars().count() < 8 {
        return Err("la contraseña debe tener al menos 8 caracteres".into());
    }
    if !ROLES.contains(&rol) {
        return Err("rol inválido".into());
    }
    let sal = SaltString::generate(&mut OsRng);
    let hash = e.argon().hash_password(clave.as_bytes(), &sal).map_err(|x| x.to_string())?.to_string();
    let r = sqlx::query("INSERT INTO usuarios (nombre, hash, rol) VALUES (?, ?, ?)")
        .bind(nombre).bind(hash).bind(rol).execute(&e.pool).await;
    match r {
        Ok(r) => Ok(r.last_insert_rowid()),
        Err(sqlx::Error::Database(d)) if d.is_unique_violation() => Err("ese nombre ya existe".into()),
        Err(x) => Err(x.to_string()),
    }
}

// ---------------------------------------------------------------- sesión (extractor)

/// Usuario autenticado de la petición actual.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Sesion {
    pub usuario_id: i64,
    pub nombre: String,
    pub rol: String,
    #[serde(skip)]
    pub csrf: String,
    #[serde(skip)]
    pub token_hash: String,
}

impl Sesion {
    fn es_admin(&self) -> bool {
        self.rol == "admin"
    }
}

/// Rechazo del extractor: los navegadores van al login.
pub struct SinSesion;
impl IntoResponse for SinSesion {
    fn into_response(self) -> Response {
        Redirect::to("/login").into_response()
    }
}

impl FromRequestParts<Estado> for Sesion {
    type Rejection = SinSesion;

    async fn from_request_parts(p: &mut Parts, e: &Estado) -> Result<Self, Self::Rejection> {
        let token = leer_cookie(&p.headers, COOKIE).ok_or(SinSesion)?;
        let th = sha256_hex(&token);
        // JOIN con usuarios: si el usuario fue desactivado, la sesión deja de valer.
        sqlx::query_as::<_, Sesion>(
            "SELECT s.usuario_id, u.nombre, u.rol, s.csrf, s.token_hash
             FROM sesiones s JOIN usuarios u ON u.id = s.usuario_id
             WHERE s.token_hash = ? AND s.expira > ? AND u.activo = 1",
        )
        .bind(th).bind(e.ahora())
        .fetch_optional(&e.pool).await.ok().flatten().ok_or(SinSesion)
    }
}

fn prohibido(m: &str) -> Response {
    (StatusCode::FORBIDDEN, Html(pagina("Prohibido", &format!("<p class=error>{}</p><a href=/panel>Volver</a>", esc(m))))).into_response()
}

/// Verifica el token CSRF del formulario contra el de la sesión.
fn chequear_csrf(s: &Sesion, enviado: &str) -> Result<(), Response> {
    if iguales_ct(&s.csrf, enviado) { Ok(()) } else { Err(prohibido("token CSRF inválido")) }
}

// ---------------------------------------------------------------- router

pub fn app(e: Estado) -> Router {
    Router::new()
        .route("/", get(|| async { Redirect::to("/panel") }))
        .route("/login", get(form_login).post(login))
        .route("/logout", post(logout))
        .route("/panel", get(panel))
        .route("/api/yo", get(|s: Sesion| async move { Json(s) }))
        .route("/panel/usuarios", post(alta_usuario))
        .route("/panel/usuarios/{id}/rol", post(cambiar_rol))
        .route("/panel/usuarios/{id}/activo", post(alternar_activo))
        .with_state(e)
}

fn pagina(titulo: &str, cuerpo: &str) -> String {
    format!(
        "<!doctype html><html lang=es><head><meta charset=utf-8><title>{t}</title>\
         <style>body{{font-family:sans-serif;margin:2em}}table{{border-collapse:collapse}}td,th{{border:1px solid #ccc;padding:4px 8px}}.error{{color:#b00}}</style>\
         </head><body><h1>{t}</h1>{cuerpo}</body></html>",
        t = esc(titulo)
    )
}

fn html_login(error: Option<&str>) -> String {
    let e = error.map(|m| format!("<p class=error>{}</p>", esc(m))).unwrap_or_default();
    pagina("Ingresar", &format!(
        "{e}<form method=post action=/login>\
         <label>Usuario <input name=nombre required></label> \
         <label>Contraseña <input name=clave type=password required></label> \
         <button>Entrar</button></form>"
    ))
}

async fn form_login() -> Html<String> {
    Html(html_login(None))
}

#[derive(Deserialize)]
pub struct Credenciales {
    nombre: String,
    clave: String,
}

async fn login(State(e): State<Estado>, Form(c): Form<Credenciales>) -> Response {
    let fila: Option<(i64, String, bool)> =
        sqlx::query_as("SELECT id, hash, activo FROM usuarios WHERE nombre = ?")
            .bind(&c.nombre).fetch_optional(&e.pool).await.ok().flatten();
    // Mensaje genérico: no revelamos si el usuario existe o está inactivo.
    let fallo = || (StatusCode::UNAUTHORIZED, Html(html_login(Some("Usuario o contraseña incorrectos")))).into_response();
    let Some((id, hash, activo)) = fila else { return fallo() };
    let ok = PasswordHash::new(&hash).map(|h| e.argon().verify_password(c.clave.as_bytes(), &h).is_ok()).unwrap_or(false);
    if !ok || !activo {
        auditar(&e.pool, e.ahora(), &c.nombre, "login fallido").await;
        return fallo();
    }
    let token = token_aleatorio();
    let ahora = e.ahora();
    // Limpieza oportunista de sesiones vencidas.
    let _ = sqlx::query("DELETE FROM sesiones WHERE expira <= ?").bind(ahora).execute(&e.pool).await;
    let r = sqlx::query("INSERT INTO sesiones (token_hash, usuario_id, csrf, expira) VALUES (?, ?, ?, ?)")
        .bind(sha256_hex(&token)).bind(id).bind(token_aleatorio()).bind(ahora + e.ttl)
        .execute(&e.pool).await;
    if r.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let _ = sqlx::query("UPDATE usuarios SET ultimo_login = ? WHERE id = ?").bind(ahora).bind(id).execute(&e.pool).await;
    auditar(&e.pool, ahora, &c.nombre, "login").await;
    let cookie = format!(
        "{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
        e.ttl,
        if e.cookie_segura { "; Secure" } else { "" }
    );
    ([(header::SET_COOKIE, cookie)], Redirect::to("/panel")).into_response()
}

#[derive(Deserialize)]
pub struct SoloCsrf {
    csrf: String,
}

async fn logout(State(e): State<Estado>, s: Sesion, Form(f): Form<SoloCsrf>) -> Response {
    if let Err(r) = chequear_csrf(&s, &f.csrf) {
        return r;
    }
    let _ = sqlx::query("DELETE FROM sesiones WHERE token_hash = ?").bind(&s.token_hash).execute(&e.pool).await;
    auditar(&e.pool, e.ahora(), &s.nombre, "logout").await;
    let borrar = format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0");
    ([(header::SET_COOKIE, borrar)], Redirect::to("/login")).into_response()
}

#[derive(sqlx::FromRow)]
struct FilaUsuario {
    id: i64,
    nombre: String,
    rol: String,
    activo: bool,
    ultimo_login: Option<i64>,
}

async fn panel(State(e): State<Estado>, s: Sesion) -> Response {
    let usuarios: Vec<FilaUsuario> =
        sqlx::query_as("SELECT id, nombre, rol, activo, ultimo_login FROM usuarios ORDER BY id")
            .fetch_all(&e.pool).await.unwrap_or_default();
    let sesiones: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sesiones WHERE expira > ?")
        .bind(e.ahora()).fetch_one(&e.pool).await.unwrap_or(0);
    let activos = usuarios.iter().filter(|u| u.activo).count();
    let csrf = esc(&s.csrf);

    let mut h = format!(
        "<p>Hola <b>{}</b> ({}). <form method=post action=/logout style=display:inline>\
         <input type=hidden name=csrf value=\"{csrf}\"><button>Salir</button></form></p>\
         <h2>Métricas</h2><ul><li>Usuarios: {}</li><li>Activos: {activos}</li><li>Sesiones abiertas: {sesiones}</li></ul>",
        esc(&s.nombre), esc(&s.rol), usuarios.len()
    );

    h.push_str("<h2>Usuarios</h2><table><tr><th>ID</th><th>Nombre</th><th>Rol</th><th>Activo</th><th>Último login</th>");
    if s.es_admin() {
        h.push_str("<th>Acciones</th>");
    }
    h.push_str("</tr>");
    for u in &usuarios {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>",
            u.id, esc(&u.nombre), esc(&u.rol), if u.activo { "sí" } else { "no" },
            u.ultimo_login.map(|t| t.to_string()).unwrap_or_else(|| "—".into())
        ));
        if s.es_admin() {
            let opciones: String = ROLES.iter()
                .map(|r| format!("<option{}>{r}</option>", if *r == u.rol { " selected" } else { "" }))
                .collect();
            h.push_str(&format!(
                "<td><form method=post action=/panel/usuarios/{id}/rol style=display:inline>\
                 <input type=hidden name=csrf value=\"{csrf}\"><select name=rol>{opciones}</select><button>Cambiar</button></form> \
                 <form method=post action=/panel/usuarios/{id}/activo style=display:inline>\
                 <input type=hidden name=csrf value=\"{csrf}\"><button>{}</button></form></td>",
                if u.activo { "Desactivar" } else { "Activar" }, id = u.id
            ));
        }
        h.push_str("</tr>");
    }
    h.push_str("</table>");

    if s.es_admin() {
        h.push_str(&format!(
            "<h2>Nuevo usuario</h2><form method=post action=/panel/usuarios>\
             <input type=hidden name=csrf value=\"{csrf}\">\
             <input name=nombre placeholder=nombre required> <input name=clave type=password placeholder=contraseña required> \
             <select name=rol><option>lector</option><option>editor</option><option>admin</option></select> <button>Crear</button></form>"
        ));
        let log: Vec<(i64, String, String)> =
            sqlx::query_as("SELECT ts, actor, accion FROM auditoria ORDER BY id DESC LIMIT 20")
                .fetch_all(&e.pool).await.unwrap_or_default();
        h.push_str("<h2>Auditoría (últimos 20)</h2><ul>");
        for (ts, actor, accion) in log {
            h.push_str(&format!("<li>{ts} · {} · {}</li>", esc(&actor), esc(&accion)));
        }
        h.push_str("</ul>");
    }
    // `no-store`: el panel tiene datos sensibles, que no quede en caché.
    ([(header::CACHE_CONTROL, "no-store")], Html(pagina("Panel de administración", &h))).into_response()
}

#[derive(Deserialize)]
pub struct AltaUsuario {
    csrf: String,
    nombre: String,
    clave: String,
    rol: String,
}

async fn alta_usuario(State(e): State<Estado>, s: Sesion, Form(f): Form<AltaUsuario>) -> Response {
    if let Err(r) = chequear_csrf(&s, &f.csrf) {
        return r;
    }
    if !s.es_admin() {
        return prohibido("sólo un admin puede crear usuarios");
    }
    match crear_usuario(&e, f.nombre.trim(), &f.clave, &f.rol).await {
        Ok(_) => {
            auditar(&e.pool, e.ahora(), &s.nombre, &format!("creó usuario {} ({})", f.nombre.trim(), f.rol)).await;
            Redirect::to("/panel").into_response()
        }
        Err(m) => (StatusCode::UNPROCESSABLE_ENTITY, Html(pagina("Error", &format!("<p class=error>{}</p><a href=/panel>Volver</a>", esc(&m))))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct CambioRol {
    csrf: String,
    rol: String,
}

/// ¿Quedaría el sistema sin ningún admin activo si sacamos a `id`?
async fn es_ultimo_admin(e: &Estado, id: i64) -> bool {
    let otros: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usuarios WHERE rol='admin' AND activo=1 AND id != ?")
        .bind(id).fetch_one(&e.pool).await.unwrap_or(0);
    otros == 0
}

async fn cambiar_rol(State(e): State<Estado>, s: Sesion, Path(id): Path<i64>, Form(f): Form<CambioRol>) -> Response {
    if let Err(r) = chequear_csrf(&s, &f.csrf) {
        return r;
    }
    if !s.es_admin() {
        return prohibido("sólo un admin puede cambiar roles");
    }
    if !ROLES.contains(&f.rol.as_str()) {
        return (StatusCode::UNPROCESSABLE_ENTITY, "rol inválido").into_response();
    }
    if f.rol != "admin" && es_ultimo_admin(&e, id).await {
        return prohibido("no se puede quitar el último admin activo");
    }
    let r = sqlx::query("UPDATE usuarios SET rol = ? WHERE id = ?").bind(&f.rol).bind(id).execute(&e.pool).await;
    if r.map(|r| r.rows_affected()).unwrap_or(0) == 0 {
        return StatusCode::NOT_FOUND.into_response();
    }
    auditar(&e.pool, e.ahora(), &s.nombre, &format!("rol de #{id} → {}", f.rol)).await;
    Redirect::to("/panel").into_response()
}

async fn alternar_activo(State(e): State<Estado>, s: Sesion, Path(id): Path<i64>, Form(f): Form<SoloCsrf>) -> Response {
    if let Err(r) = chequear_csrf(&s, &f.csrf) {
        return r;
    }
    if !s.es_admin() {
        return prohibido("sólo un admin puede activar/desactivar");
    }
    if id == s.usuario_id {
        return prohibido("no podés desactivarte a vos mismo");
    }
    let r: Option<bool> = sqlx::query_scalar("UPDATE usuarios SET activo = 1 - activo WHERE id = ? RETURNING activo")
        .bind(id).fetch_optional(&e.pool).await.ok().flatten();
    let Some(activo) = r else { return StatusCode::NOT_FOUND.into_response() };
    if !activo {
        // Revocamos sus sesiones inmediatamente.
        let _ = sqlx::query("DELETE FROM sesiones WHERE usuario_id = ?").bind(id).execute(&e.pool).await;
    }
    auditar(&e.pool, e.ahora(), &s.nombre, &format!("#{id} activo={activo}")).await;
    Redirect::to("/panel").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utilidades() {
        assert_eq!(esc(r#"<a href="x">'&'</a>"#), "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;");
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, "tema=oscuro; sid=abc123; x=1".parse().unwrap());
        assert_eq!(leer_cookie(&h, "sid").as_deref(), Some("abc123"));
        assert_eq!(leer_cookie(&h, "otra"), None);
        assert!(iguales_ct("abc", "abc") && !iguales_ct("abc", "abd") && !iguales_ct("ab", "abc"));
        assert!(nombre_valido("ana.lopez") && !nombre_valido("ab") && !nombre_valido("a b c"));
        assert_ne!(token_aleatorio(), token_aleatorio());
        assert_eq!(token_aleatorio().len(), 64);
    }
}
