//! Tests del flujo de autenticación completo, sin red.

use argon2::Params;
use auth_jwt::{app, crear_pool, emitir, verificar, Config, Estado};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

fn cfg(duracion: u64) -> Config {
    // Argon2 liviano para que los tests sean rápidos en modo debug.
    Config { secreto: b"secreto-de-pruebas-de-32-bytes!!".to_vec(), duracion, argon: Params::new(64, 1, 1, None).unwrap() }
}

async fn nueva() -> axum::Router {
    app(Estado { db: crear_pool("sqlite::memory:").await.unwrap(), cfg: Arc::new(cfg(60)) })
}

async fn pedir(a: &axum::Router, m: &str, uri: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
    let mut b = Request::builder().method(m).uri(uri);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let cuerpo = match body {
        Some(v) => {
            b = b.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let r = a.clone().oneshot(b.body(cuerpo).unwrap()).await.unwrap();
    let st = r.status();
    let by = r.into_body().collect().await.unwrap().to_bytes();
    (st, serde_json::from_slice(&by).unwrap_or(Value::Null))
}

async fn token_de(a: &axum::Router, email: &str) -> String {
    pedir(a, "POST", "/auth/registro", None, Some(json!({"email": email, "clave": "clave-segura"}))).await;
    let (st, v) = pedir(a, "POST", "/auth/login", None, Some(json!({"email": email, "clave": "clave-segura"}))).await;
    assert_eq!(st, StatusCode::OK);
    v["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn registro_login_y_ruta_protegida() {
    let a = nueva().await;
    let (st, v) = pedir(&a, "POST", "/auth/registro", None, Some(json!({"email": "ana@x.com", "clave": "clave-segura"}))).await;
    assert_eq!(st, StatusCode::CREATED);
    assert_eq!(v["rol"], "admin"); // primer usuario
    let t = token_de(&a, "ana@x.com").await;
    let (st, v) = pedir(&a, "GET", "/yo", Some(&t), None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["email"], "ana@x.com");
}

#[tokio::test]
async fn rechazos_de_autenticacion() {
    let a = nueva().await;
    assert_eq!(pedir(&a, "GET", "/yo", None, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(pedir(&a, "GET", "/yo", Some("basura"), None).await.0, StatusCode::UNAUTHORIZED);
    // Token firmado con otro secreto.
    let mut otro = cfg(60);
    otro.secreto = b"otro-secreto-otro-secreto-otro-!!".to_vec();
    let falso = emitir(&otro, 1, "ana@x.com", "admin");
    assert_eq!(pedir(&a, "GET", "/yo", Some(&falso), None).await.0, StatusCode::UNAUTHORIZED);
    // Login con clave incorrecta y con email inexistente: mismo error.
    pedir(&a, "POST", "/auth/registro", None, Some(json!({"email": "b@x.com", "clave": "clave-segura"}))).await;
    let (s1, v1) = pedir(&a, "POST", "/auth/login", None, Some(json!({"email": "b@x.com", "clave": "incorrecta"}))).await;
    let (s2, v2) = pedir(&a, "POST", "/auth/login", None, Some(json!({"email": "zz@x.com", "clave": "incorrecta"}))).await;
    assert_eq!((s1, s2), (StatusCode::UNAUTHORIZED, StatusCode::UNAUTHORIZED));
    assert_eq!(v1, v2);
}

#[tokio::test]
async fn token_expirado() {
    let c = cfg(0); // expira en el mismo segundo
    let t = emitir(&c, 1, "a@b.co", "usuario");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let e = verificar(&c, &t).unwrap_err();
    assert_eq!(*e.kind(), jsonwebtoken::errors::ErrorKind::ExpiredSignature);
}

#[tokio::test]
async fn validaciones_y_duplicados() {
    let a = nueva().await;
    let (st, _) = pedir(&a, "POST", "/auth/registro", None, Some(json!({"email": "sin-arroba", "clave": "clave-segura"}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = pedir(&a, "POST", "/auth/registro", None, Some(json!({"email": "c@x.com", "clave": "corta"}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    pedir(&a, "POST", "/auth/registro", None, Some(json!({"email": "c@x.com", "clave": "clave-segura"}))).await;
    // Email case-insensitive (COLLATE NOCASE).
    let (st, _) = pedir(&a, "POST", "/auth/registro", None, Some(json!({"email": "C@X.com", "clave": "clave-segura"}))).await;
    assert_eq!(st, StatusCode::CONFLICT);
}

#[tokio::test]
async fn roles() {
    let a = nueva().await;
    let admin = token_de(&a, "admin@x.com").await;
    let comun = token_de(&a, "comun@x.com").await;
    assert_eq!(pedir(&a, "GET", "/admin/usuarios", Some(&comun), None).await.0, StatusCode::FORBIDDEN);
    let (st, v) = pedir(&a, "GET", "/admin/usuarios", Some(&admin), None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v.as_array().unwrap().len(), 2);
    assert!(v.to_string().find("hash").is_none()); // nunca exponer hashes
}
