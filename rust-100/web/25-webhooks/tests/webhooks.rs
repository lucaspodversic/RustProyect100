//! Integración: recepción vía oneshot + reenvío a destinos HTTP locales reales.

use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use webhooks::*;

const T0: i64 = 1_800_000_000;

/// Destino falso: falla las primeras `fallar` veces, luego 200. Guarda lo recibido.
async fn destino(fallar: usize) -> (String, Arc<Mutex<Vec<(HeaderMap, Bytes)>>>) {
    let recibidos = Arc::new(Mutex::new(vec![]));
    let cont = Arc::new(AtomicUsize::new(0));
    let r2 = recibidos.clone();
    let app = axum::Router::new().route(
        "/hook",
        axum::routing::post(move |h: HeaderMap, b: Bytes| async move {
            if cont.fetch_add(1, Ordering::SeqCst) < fallar {
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
            r2.lock().unwrap().push((h, b));
            StatusCode::OK
        }),
    );
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let d = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (format!("http://{d}/hook"), recibidos)
}

async fn preparar(destinos: Vec<String>) -> (Estado, Arc<AtomicI64>) {
    let t = Arc::new(AtomicI64::new(T0));
    let t2 = t.clone();
    let cfg = Config {
        secretos: HashMap::from([("pagos".into(), "s3cr3t".into())]),
        destinos: HashMap::from([("pagos".into(), destinos)]),
        secreto_salida: "salida".into(),
        tolerancia_seg: 300,
        max_intentos: 3,
        backoff_base_seg: 10,
        max_cuerpo: 1024,
    };
    (Estado::new(crear_pool("sqlite::memory:").await.unwrap(), cfg, Arc::new(move || t2.load(Ordering::SeqCst))), t)
}

fn webhook(id: &str, cuerpo: &str, secreto: &str, ts: i64) -> Request<Body> {
    Request::post("/webhooks/pagos")
        .header("x-evento-id", id)
        .header("x-timestamp", ts.to_string())
        .header("x-firma", firma::firmar(secreto.as_bytes(), ts, cuerpo.as_bytes()))
        .body(Body::from(cuerpo.to_string()))
        .unwrap()
}

async fn enviar(e: &Estado, r: Request<Body>) -> (StatusCode, Value) {
    let res = app(e.clone()).oneshot(r).await.unwrap();
    let s = res.status();
    (s, serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap_or(Value::Null))
}

#[tokio::test]
async fn recepcion_verifica_y_es_idempotente() {
    let (e, _) = preparar(vec![]).await;
    assert_eq!(enviar(&e, webhook("ev1", r#"{"monto":10}"#, "s3cr3t", T0)).await.0, StatusCode::ACCEPTED);
    let (s, v) = enviar(&e, webhook("ev1", r#"{"monto":10}"#, "s3cr3t", T0)).await;
    assert_eq!((s, v["duplicado"].as_bool()), (StatusCode::OK, Some(true)));
    assert_eq!(enviar(&e, webhook("ev2", "{}", "malo", T0)).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(enviar(&e, webhook("ev3", "{}", "s3cr3t", T0 - 301)).await.0, StatusCode::UNAUTHORIZED); // replay viejo
    let sin_firma = Request::post("/webhooks/pagos").body(Body::from("{}")).unwrap();
    assert_eq!(enviar(&e, sin_firma).await.0, StatusCode::BAD_REQUEST);
    let otra = Request::post("/webhooks/nada").body(Body::from("{}")).unwrap();
    assert_eq!(enviar(&e, otra).await.0, StatusCode::NOT_FOUND);
    let grande = "x".repeat(2000);
    assert_eq!(enviar(&e, webhook("ev4", &grande, "s3cr3t", T0)).await.0, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn reenvio_con_reintentos_backoff_y_firma_de_salida() {
    let (url_ok, rec_ok) = destino(0).await;
    let (url_flojo, rec_flojo) = destino(1).await; // falla 1 vez
    let (e, t) = preparar(vec![url_ok, url_flojo]).await;
    let (_, v) = enviar(&e, webhook("ev1", r#"{"a":1}"#, "s3cr3t", T0)).await;
    let id = v["id"].as_i64().unwrap();

    assert_eq!(procesar_pendientes(&e).await.unwrap(), 2);
    assert_eq!(rec_ok.lock().unwrap().len(), 1);
    assert_eq!(rec_flojo.lock().unwrap().len(), 0);
    // Antes del backoff (10 s) no se reintenta.
    t.fetch_add(5, Ordering::SeqCst);
    assert_eq!(procesar_pendientes(&e).await.unwrap(), 0);
    t.fetch_add(5, Ordering::SeqCst);
    assert_eq!(procesar_pendientes(&e).await.unwrap(), 1);
    assert_eq!(rec_flojo.lock().unwrap().len(), 1);

    // El destino puede verificar nuestra firma de salida.
    let (h, b) = rec_ok.lock().unwrap()[0].clone();
    let ts: i64 = h["x-timestamp"].to_str().unwrap().parse().unwrap();
    assert!(firma::verificar(b"salida", ts, &b, h["x-firma"].to_str().unwrap(), ts, 1).is_ok());
    assert_eq!(&b[..], br#"{"a":1}"#);

    let (_, ent) = enviar(&e, Request::get(format!("/eventos/{id}/entregas")).body(Body::empty()).unwrap()).await;
    assert_eq!(ent[0]["estado"], "entregada");
    assert_eq!((ent[1]["estado"].as_str(), ent[1]["intentos"].as_i64()), (Some("entregada"), Some(2)));
}

#[tokio::test]
async fn dead_letter_y_reintento_manual() {
    // Puerto 1: conexión rechazada siempre.
    let (e, t) = preparar(vec!["http://127.0.0.1:1/hook".into()]).await;
    enviar(&e, webhook("ev1", "{}", "s3cr3t", T0)).await;
    for _ in 0..3 {
        procesar_pendientes(&e).await.unwrap();
        t.fetch_add(10_000, Ordering::SeqCst);
    }
    let (_, ent) = enviar(&e, Request::get("/eventos/1/entregas").body(Body::empty()).unwrap()).await;
    assert_eq!((ent[0]["estado"].as_str(), ent[0]["intentos"].as_i64()), (Some("fallida"), Some(3)));
    assert!(ent[0]["ultimo_error"].as_str().unwrap().starts_with("red:"));
    assert_eq!(procesar_pendientes(&e).await.unwrap(), 0); // ya no se intenta

    assert_eq!(enviar(&e, Request::post("/entregas/1/reintentar").body(Body::empty()).unwrap()).await.0, StatusCode::NO_CONTENT);
    assert_eq!(enviar(&e, Request::post("/entregas/1/reintentar").body(Body::empty()).unwrap()).await.0, StatusCode::CONFLICT);
    assert_eq!(procesar_pendientes(&e).await.unwrap(), 1);
}
