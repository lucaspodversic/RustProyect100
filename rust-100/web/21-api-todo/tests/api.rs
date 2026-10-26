//! Tests de la API To-Do vía oneshot + SQLite en memoria.

use api_todo::{app, crear_pool, fecha_valida};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn pedir(a: &axum::Router, m: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut b = Request::builder().method(m).uri(uri);
    let c = match body {
        Some(v) => {
            b = b.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let r = a.clone().oneshot(b.body(c).unwrap()).await.unwrap();
    let st = r.status();
    let by = r.into_body().collect().await.unwrap().to_bytes();
    (st, serde_json::from_slice(&by).unwrap_or(Value::Null))
}

async fn nueva() -> axum::Router {
    app(crear_pool("sqlite::memory:").await.unwrap())
}

#[test]
fn fechas() {
    assert!(fecha_valida("2024-02-29"));
    assert!(!fecha_valida("2023-02-29"));
    assert!(!fecha_valida("2026-13-01"));
    assert!(!fecha_valida("26-1-1"));
}

#[tokio::test]
async fn listas_items_y_conteos() {
    let a = nueva().await;
    let (st, l) = pedir(&a, "POST", "/listas", Some(json!({"nombre": "Súper"}))).await;
    assert_eq!(st, StatusCode::CREATED);
    let id = l["id"].as_i64().unwrap();
    for t in ["Yerba", "Pan", "Leche"] {
        pedir(&a, "POST", &format!("/listas/{id}/items"), Some(json!({"titulo": t}))).await;
    }
    let (st, it) = pedir(&a, "PATCH", &format!("/listas/{id}/items/2"), Some(json!({"completado": true}))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(it["completado"], true);
    assert!(it["completado_en"].is_string());
    assert_eq!(it["titulo"], "Pan"); // no se tocó

    let (_, ls) = pedir(&a, "GET", "/listas", None).await;
    assert_eq!((ls[0]["total"].as_i64(), ls[0]["pendientes"].as_i64()), (Some(3), Some(2)));
    let (_, pend) = pedir(&a, "GET", &format!("/listas/{id}/items?completado=false"), None).await;
    assert_eq!(pend.as_array().unwrap().len(), 2);

    // Des-completar limpia completado_en.
    let (_, it) = pedir(&a, "PATCH", &format!("/listas/{id}/items/2"), Some(json!({"completado": false}))).await;
    assert!(it["completado_en"].is_null());

    let (_, r) = pedir(&a, "POST", &format!("/listas/{id}/completar-todo"), None).await;
    assert_eq!(r["completados"], 3);
}

#[tokio::test]
async fn patch_vence_tres_estados() {
    let a = nueva().await;
    pedir(&a, "POST", "/listas", Some(json!({"nombre": "L"}))).await;
    pedir(&a, "POST", "/listas/1/items", Some(json!({"titulo": "x", "vence": "2026-12-31"}))).await;
    let (_, it) = pedir(&a, "PATCH", "/listas/1/items/1", Some(json!({"titulo": "y"}))).await;
    assert_eq!(it["vence"], "2026-12-31"); // ausente → se mantiene
    let (_, it) = pedir(&a, "PATCH", "/listas/1/items/1", Some(json!({"vence": null}))).await;
    assert!(it["vence"].is_null()); // null → se borra
    let (st, _) = pedir(&a, "PATCH", "/listas/1/items/1", Some(json!({"vence": "2026-02-30"}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn aislamiento_entre_listas_y_cascada() {
    let a = nueva().await;
    pedir(&a, "POST", "/listas", Some(json!({"nombre": "A"}))).await;
    pedir(&a, "POST", "/listas", Some(json!({"nombre": "B"}))).await;
    pedir(&a, "POST", "/listas/1/items", Some(json!({"titulo": "de A"}))).await;
    // Ítem 1 pertenece a A: vía B no se encuentra.
    assert_eq!(pedir(&a, "PATCH", "/listas/2/items/1", Some(json!({"completado": true}))).await.0, StatusCode::NOT_FOUND);
    assert_eq!(pedir(&a, "DELETE", "/listas/2/items/1", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(pedir(&a, "POST", "/listas/99/items", Some(json!({"titulo": "x"}))).await.0, StatusCode::NOT_FOUND);
    assert_eq!(pedir(&a, "DELETE", "/listas/1", None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(pedir(&a, "GET", "/listas/1/items", None).await.0, StatusCode::NOT_FOUND);
    let (_, d) = pedir(&a, "GET", "/listas/2", None).await;
    assert_eq!(d["items"].as_array().unwrap().len(), 0);
}
