//! Tests con reloj controlado para simular clics en distintos días.

use acortador_analiticas::{app, crear_pool, Estado};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

const D1: i64 = 1_793_016_000; // 2026-10-26 12:00 UTC

async fn preparar() -> (axum::Router, Arc<AtomicI64>) {
    let t = Arc::new(AtomicI64::new(D1));
    let t2 = t.clone();
    let e = Estado { db: crear_pool("sqlite::memory:").await.unwrap(), reloj: Arc::new(move || t2.load(Ordering::SeqCst)), sal: "test".into() };
    (app(e), t)
}

async fn enviar(a: &axum::Router, r: Request<Body>) -> (StatusCode, axum::http::HeaderMap, Value) {
    let res = a.clone().oneshot(r).await.unwrap();
    let (p, b) = res.into_parts();
    (p.status, p.headers, serde_json::from_slice(&b.collect().await.unwrap().to_bytes()).unwrap_or(Value::Null))
}

fn clic(cod: &str, ip: &str, ua: &str, referer: Option<&str>) -> Request<Body> {
    let mut b = Request::get(format!("/{cod}")).header("x-forwarded-for", ip).header("user-agent", ua);
    if let Some(r) = referer {
        b = b.header("referer", r);
    }
    b.body(Body::empty()).unwrap()
}

fn crear(url: &str, codigo: Option<&str>) -> Request<Body> {
    Request::post("/api/enlaces").header("content-type", "application/json").body(Body::from(json!({"url": url, "codigo": codigo}).to_string())).unwrap()
}

#[tokio::test]
async fn clics_por_dia_referrers_dispositivos_unicos() {
    let (a, t) = preparar().await;
    assert_eq!(enviar(&a, crear("https://rust-lang.org", Some("rust"))).await.0, StatusCode::CREATED);
    const PC: &str = "Mozilla/5.0 (X11; Linux)";
    const CEL: &str = "Mozilla/5.0 (iPhone)";

    // Día 1: ana dos veces (1 único), beto desde el celu con referer de Twitter.
    let (s, h, _) = enviar(&a, clic("rust", "1.1.1.1", PC, Some("https://www.google.com/search?q=x"))).await;
    assert_eq!(s, StatusCode::FOUND);
    assert_eq!(h["location"], "https://rust-lang.org/");
    enviar(&a, clic("rust", "1.1.1.1", PC, Some("https://google.com/"))).await;
    enviar(&a, clic("rust", "2.2.2.2", CEL, Some("https://x.com/post/1"))).await;
    enviar(&a, clic("rust", "3.3.3.3", "Googlebot/2.1", None)).await; // bot: no cuenta
    // Día 3 (salteamos el día 2).
    t.store(D1 + 2 * 86_400, Ordering::SeqCst);
    enviar(&a, clic("rust", "1.1.1.1", PC, None)).await;

    let (s, _, v) = enviar(&a, Request::get("/api/enlaces/rust/analiticas?desde=2026-10-26&hasta=2026-10-28").body(Body::empty()).unwrap()).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["total"], 4);
    assert_eq!(v["unicos"], 3); // ana día1, beto día1, ana día3 (el hash cambia por día)
    assert_eq!(v["bots"], 1);
    assert_eq!(v["por_dia"], json!([{"dia":"2026-10-26","clics":3},{"dia":"2026-10-27","clics":0},{"dia":"2026-10-28","clics":1}]));
    assert_eq!(v["referrers"][0], json!({"clave":"google.com","clics":2}));
    assert_eq!(v["dispositivos"][0], json!({"clave":"escritorio","clics":3}));

    // Rango por defecto (7 días hasta "hoy" = día 3).
    let (_, _, v) = enviar(&a, Request::get("/api/enlaces/rust/analiticas").body(Body::empty()).unwrap()).await;
    assert_eq!(v["por_dia"].as_array().unwrap().len(), 7);
    assert_eq!(v["hasta"], "2026-10-28");
}

#[tokio::test]
async fn validaciones_y_errores() {
    let (a, _) = preparar().await;
    assert_eq!(enviar(&a, crear("javascript:alert(1)", None)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(enviar(&a, crear("https://a.com", Some("api"))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let (s, _, v) = enviar(&a, crear("https://a.com", None)).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(v["codigo"].as_str().unwrap().len(), 7);
    enviar(&a, crear("https://a.com", Some("mio"))).await;
    assert_eq!(enviar(&a, crear("https://b.com", Some("mio"))).await.0, StatusCode::CONFLICT);
    assert_eq!(enviar(&a, clic("nada", "1", "x", None)).await.0, StatusCode::NOT_FOUND);
    let r = Request::get("/api/enlaces/mio/analiticas?desde=2026-10-30&hasta=2026-10-01").body(Body::empty()).unwrap();
    assert_eq!(enviar(&a, r).await.0, StatusCode::BAD_REQUEST);
}
