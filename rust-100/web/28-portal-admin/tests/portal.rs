//! Tests de integración del portal: SQLite en memoria, Argon2 liviano y reloj falso.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use portal_admin::{app, crear_pool, crear_usuario, Estado};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

struct Ctx {
    app: axum::Router,
    e: Estado,
    reloj: Arc<AtomicI64>,
}

async fn ctx() -> Ctx {
    let mut e = Estado::nuevo(crear_pool("sqlite::memory:").await.unwrap());
    e.argon = argon2::Params::new(8, 1, 1, None).unwrap(); // rápido para tests
    let reloj = Arc::new(AtomicI64::new(1_000_000));
    let r = reloj.clone();
    e.reloj = Arc::new(move || r.load(Ordering::SeqCst));
    e.ttl = 3600;
    crear_usuario(&e, "admin", "clave-admin", "admin").await.unwrap();
    crear_usuario(&e, "edi", "clave-editor", "editor").await.unwrap();
    Ctx { app: app(e.clone()), e, reloj }
}

struct Resp {
    status: StatusCode,
    location: Option<String>,
    set_cookie: Option<String>,
    body: String,
}

async fn pedir(c: &Ctx, metodo: &str, uri: &str, cookie: Option<&str>, form: Option<&str>) -> Resp {
    let mut b = Request::builder().method(metodo).uri(uri);
    if let Some(ck) = cookie {
        b = b.header(header::COOKIE, format!("sid={ck}"));
    }
    if form.is_some() {
        b = b.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    }
    let r = c.app.clone().oneshot(b.body(Body::from(form.unwrap_or("").to_string())).unwrap()).await.unwrap();
    let h = |n| r.headers().get(n).map(|v: &header::HeaderValue| v.to_str().unwrap().to_string());
    let (status, location, set_cookie) = (r.status(), h(header::LOCATION), h(header::SET_COOKIE));
    let body = String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    Resp { status, location, set_cookie, body }
}

/// Hace login y devuelve (token de cookie, token CSRF leído del panel).
async fn entrar(c: &Ctx, nombre: &str, clave: &str) -> (String, String) {
    let r = pedir(c, "POST", "/login", None, Some(&format!("nombre={nombre}&clave={clave}"))).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.body);
    let ck = r.set_cookie.unwrap();
    assert!(ck.contains("HttpOnly") && ck.contains("SameSite=Strict"));
    let token = ck.trim_start_matches("sid=").split(';').next().unwrap().to_string();
    let panel = pedir(c, "GET", "/panel", Some(&token), None).await;
    let csrf = panel.body.split("name=csrf value=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
    (token, csrf)
}

#[tokio::test]
async fn login_y_proteccion_de_rutas() {
    let c = ctx().await;
    // Sin sesión → redirige al login.
    let r = pedir(&c, "GET", "/panel", None, None).await;
    assert_eq!(r.location.as_deref(), Some("/login"));
    // Cookie inventada → también.
    assert_eq!(pedir(&c, "GET", "/panel", Some("falsa"), None).await.location.as_deref(), Some("/login"));
    // Clave incorrecta y usuario inexistente → mismo 401 genérico.
    let a = pedir(&c, "POST", "/login", None, Some("nombre=admin&clave=mal")).await;
    let b = pedir(&c, "POST", "/login", None, Some("nombre=nadie&clave=mal")).await;
    assert_eq!((a.status, b.status), (StatusCode::UNAUTHORIZED, StatusCode::UNAUTHORIZED));
    assert!(a.body.contains("incorrectos") && b.body.contains("incorrectos"));

    let (tok, _) = entrar(&c, "admin", "clave-admin").await;
    let p = pedir(&c, "GET", "/panel", Some(&tok), None).await;
    assert_eq!(p.status, StatusCode::OK);
    assert!(p.body.contains("Hola <b>admin</b>") && p.body.contains("Nuevo usuario"));
    assert!(p.body.contains("login fallido"), "la auditoría registra intentos fallidos");
    let yo = pedir(&c, "GET", "/api/yo", Some(&tok), None).await;
    assert!(yo.body.contains(r#""rol":"admin""#) && !yo.body.contains("csrf"));

    // La base guarda el HASH del token, nunca el token.
    let guardado: String = sqlx::query_scalar("SELECT token_hash FROM sesiones LIMIT 1").fetch_one(&c.e.pool).await.unwrap();
    assert_ne!(guardado, tok);
}

#[tokio::test]
async fn roles_y_csrf() {
    let c = ctx().await;
    let (tok_ed, csrf_ed) = entrar(&c, "edi", "clave-editor").await;
    // El editor ve el panel pero sin acciones de admin.
    let p = pedir(&c, "GET", "/panel", Some(&tok_ed), None).await;
    assert!(!p.body.contains("Nuevo usuario") && !p.body.contains("Auditoría"));
    let r = pedir(&c, "POST", "/panel/usuarios", Some(&tok_ed), Some(&format!("csrf={csrf_ed}&nombre=pepe&clave=12345678&rol=admin"))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    let (tok, csrf) = entrar(&c, "admin", "clave-admin").await;
    // Sin CSRF correcto → 403 aunque sea admin.
    let r = pedir(&c, "POST", "/panel/usuarios", Some(&tok), Some("csrf=otro&nombre=pepe&clave=12345678&rol=lector")).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // El CSRF de OTRA sesión tampoco sirve.
    let r = pedir(&c, "POST", "/panel/usuarios", Some(&tok), Some(&format!("csrf={csrf_ed}&nombre=pepe&clave=12345678&rol=lector"))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // Correcto → 303 y aparece en el panel.
    let r = pedir(&c, "POST", "/panel/usuarios", Some(&tok), Some(&format!("csrf={csrf}&nombre=pepe&clave=12345678&rol=lector"))).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    // Validación: nombre con HTML rechazado, duplicado rechazado.
    let r = pedir(&c, "POST", "/panel/usuarios", Some(&tok), Some(&format!("csrf={csrf}&nombre=%3Cscript%3E&clave=12345678&rol=lector"))).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(!r.body.contains("<script>"));
    let r = pedir(&c, "POST", "/panel/usuarios", Some(&tok), Some(&format!("csrf={csrf}&nombre=PEPE&clave=12345678&rol=lector"))).await;
    assert!(r.body.contains("ya existe"));

    // Cambiar rol (editor #2 → lector) y protección del último admin (#1).
    assert_eq!(pedir(&c, "POST", "/panel/usuarios/2/rol", Some(&tok), Some(&format!("csrf={csrf}&rol=lector"))).await.status, StatusCode::SEE_OTHER);
    assert_eq!(pedir(&c, "POST", "/panel/usuarios/1/rol", Some(&tok), Some(&format!("csrf={csrf}&rol=lector"))).await.status, StatusCode::FORBIDDEN);
    assert_eq!(pedir(&c, "POST", "/panel/usuarios/99/rol", Some(&tok), Some(&format!("csrf={csrf}&rol=lector"))).await.status, StatusCode::NOT_FOUND);
    let yo = pedir(&c, "GET", "/api/yo", Some(&tok_ed), None).await;
    assert!(yo.body.contains(r#""rol":"lector""#), "el rol nuevo aplica en la sesión existente");
}

#[tokio::test]
async fn desactivar_logout_y_expiracion() {
    let c = ctx().await;
    let (tok, csrf) = entrar(&c, "admin", "clave-admin").await;
    let (tok_ed, csrf_ed) = entrar(&c, "edi", "clave-editor").await;
    // No puede desactivarse a sí mismo.
    assert_eq!(pedir(&c, "POST", "/panel/usuarios/1/activo", Some(&tok), Some(&format!("csrf={csrf}"))).await.status, StatusCode::FORBIDDEN);
    // Desactivar al editor revoca su sesión y le impide entrar.
    assert_eq!(pedir(&c, "POST", "/panel/usuarios/2/activo", Some(&tok), Some(&format!("csrf={csrf}"))).await.status, StatusCode::SEE_OTHER);
    assert_eq!(pedir(&c, "GET", "/panel", Some(&tok_ed), None).await.location.as_deref(), Some("/login"));
    assert_eq!(pedir(&c, "POST", "/login", None, Some("nombre=edi&clave=clave-editor")).await.status, StatusCode::UNAUTHORIZED);
    let _ = csrf_ed;

    // Logout: requiere CSRF, borra la cookie y la sesión.
    assert_eq!(pedir(&c, "POST", "/logout", Some(&tok), Some("csrf=x")).await.status, StatusCode::FORBIDDEN);
    let r = pedir(&c, "POST", "/logout", Some(&tok), Some(&format!("csrf={csrf}"))).await;
    assert!(r.set_cookie.unwrap().contains("Max-Age=0"));
    assert_eq!(pedir(&c, "GET", "/panel", Some(&tok), None).await.location.as_deref(), Some("/login"));

    // Expiración por TTL con el reloj falso.
    let (tok2, _) = entrar(&c, "admin", "clave-admin").await;
    c.reloj.fetch_add(3599, Ordering::SeqCst);
    assert_eq!(pedir(&c, "GET", "/panel", Some(&tok2), None).await.status, StatusCode::OK);
    c.reloj.fetch_add(1, Ordering::SeqCst);
    assert_eq!(pedir(&c, "GET", "/panel", Some(&tok2), None).await.location.as_deref(), Some("/login"));
}
