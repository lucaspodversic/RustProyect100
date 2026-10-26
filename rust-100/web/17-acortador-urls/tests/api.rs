//! Tests del acortador vía `oneshot` (sin red) y SQLite en memoria.

use acortador_urls::{app, base62, crear_pool, validar_alias, validar_url, Estado};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn nueva() -> axum::Router {
    app(Estado { db: crear_pool("sqlite::memory:").await.unwrap(), base: "http://cor.to".into() })
}

async fn pedir(a: &axum::Router, m: &str, uri: &str, body: Option<Value>) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut b = Request::builder().method(m).uri(uri);
    let cuerpo = match body {
        Some(v) => {
            b = b.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let r = a.clone().oneshot(b.body(cuerpo).unwrap()).await.unwrap();
    let (st, h) = (r.status(), r.headers().clone());
    let by = r.into_body().collect().await.unwrap().to_bytes();
    (st, h, serde_json::from_slice(&by).unwrap_or(Value::Null))
}

#[test]
fn base62_y_validaciones() {
    assert_eq!(base62(0), "0");
    assert_eq!(base62(61), "Z");
    assert_eq!(base62(62), "10");
    assert!(validar_url("javascript:alert(1)", "http://cor.to").is_err());
    assert!(validar_url("ftp://x.com/a", "http://cor.to").is_err());
    assert!(validar_url("http://cor.to/abc", "http://cor.to").is_err());
    assert_eq!(validar_url(" https://Ejemplo.com ", "http://cor.to").unwrap(), "https://ejemplo.com/");
    assert!(validar_alias("mi-link_1").is_ok());
    assert!(validar_alias("a").is_err());
    assert!(validar_alias("API").is_err());
    assert!(validar_alias("con espacio").is_err());
}

#[tokio::test]
async fn acortar_redirigir_y_contar() {
    let a = nueva().await;
    let (st, _, v) = pedir(&a, "POST", "/api/acortar", Some(json!({"url": "https://www.rust-lang.org/learn"}))).await;
    assert_eq!(st, StatusCode::CREATED);
    let codigo = v["codigo"].as_str().unwrap().to_string();
    assert_eq!(codigo, "1001"); // id 1 + 62³ en base62
    assert_eq!(v["corta"], "http://cor.to/1001");
    for _ in 0..3 {
        let (st, h, _) = pedir(&a, "GET", &format!("/{codigo}"), None).await;
        assert_eq!(st, StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(h["location"], "https://www.rust-lang.org/learn");
    }
    let (_, _, s) = pedir(&a, "GET", &format!("/api/stats/{codigo}"), None).await;
    assert_eq!(s["clics"], 3);
    assert!(s["ultimo_acceso"].is_string());
}

#[tokio::test]
async fn alias_y_conflictos() {
    let a = nueva().await;
    let (st, _, _) = pedir(&a, "POST", "/api/acortar", Some(json!({"url": "https://a.com", "alias": "promo"}))).await;
    assert_eq!(st, StatusCode::CREATED);
    let (st, _, v) = pedir(&a, "POST", "/api/acortar", Some(json!({"url": "https://b.com", "alias": "promo"}))).await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    let (st, _, _) = pedir(&a, "POST", "/api/acortar", Some(json!({"url": "https://b.com", "alias": "api"}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, h, _) = pedir(&a, "GET", "/promo", None).await;
    assert_eq!((st, h["location"].to_str().unwrap()), (StatusCode::TEMPORARY_REDIRECT, "https://a.com/"));
}

#[tokio::test]
async fn inexistentes_y_borrado() {
    let a = nueva().await;
    assert_eq!(pedir(&a, "GET", "/nada", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(pedir(&a, "GET", "/api/stats/nada", None).await.0, StatusCode::NOT_FOUND);
    let (_, _, v) = pedir(&a, "POST", "/api/acortar", Some(json!({"url": "http://x.org"}))).await;
    let c = v["codigo"].as_str().unwrap();
    assert_eq!(pedir(&a, "DELETE", &format!("/api/{c}"), None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(pedir(&a, "GET", &format!("/{c}"), None).await.0, StatusCode::NOT_FOUND);
}
