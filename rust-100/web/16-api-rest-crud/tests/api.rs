//! Tests de la API con `tower::ServiceExt::oneshot`: se llama al Router como
//! a una función, sin abrir puertos, con SQLite en memoria.

use api_rest_crud::{app, crear_pool};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn nueva_app() -> axum::Router {
    app(crear_pool("sqlite::memory:").await.unwrap())
}

/// Ejecuta una petición y devuelve (código, cabecera Location, JSON).
async fn pedir(app: &axum::Router, metodo: &str, uri: &str, cuerpo: Option<Value>) -> (StatusCode, Option<String>, Value) {
    let mut b = Request::builder().method(metodo).uri(uri);
    let body = match cuerpo {
        Some(v) => {
            b = b.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let r = app.clone().oneshot(b.body(body).unwrap()).await.unwrap();
    let st = r.status();
    let loc = r.headers().get("location").map(|h| h.to_str().unwrap().to_string());
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    let v = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap_or(Value::Null) };
    (st, loc, v)
}

#[tokio::test]
async fn ciclo_crud_completo() {
    let a = nueva_app().await;
    let (st, loc, p) = pedir(&a, "POST", "/productos", Some(json!({"nombre": "Yerba", "precio_centavos": 350000, "stock": 10}))).await;
    assert_eq!(st, StatusCode::CREATED);
    assert_eq!(loc.as_deref(), Some("/productos/1"));
    assert_eq!(p["nombre"], "Yerba");

    let (st, _, p) = pedir(&a, "GET", "/productos/1", None).await;
    assert_eq!((st, p["stock"].as_i64()), (StatusCode::OK, Some(10)));

    let (st, _, p) = pedir(&a, "PUT", "/productos/1", Some(json!({"nombre": "Yerba 1kg", "precio_centavos": 400000}))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(p["stock"], 0); // PUT reemplaza: stock vuelve al default

    let (st, _, _) = pedir(&a, "DELETE", "/productos/1", None).await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, _, e) = pedir(&a, "GET", "/productos/1", None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(e["error"], "no encontrado");
    let (st, _, _) = pedir(&a, "DELETE", "/productos/1", None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn validacion_y_conflictos() {
    let a = nueva_app().await;
    let (st, _, _) = pedir(&a, "POST", "/productos", Some(json!({"nombre": "  ", "precio_centavos": 1}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _, _) = pedir(&a, "POST", "/productos", Some(json!({"nombre": "x", "precio_centavos": -1}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    // JSON con campo faltante: lo rechaza el extractor Json de axum (422).
    let (st, _, _) = pedir(&a, "POST", "/productos", Some(json!({"nombre": "x"}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    pedir(&a, "POST", "/productos", Some(json!({"nombre": "Mate", "precio_centavos": 1}))).await;
    let (st, _, e) = pedir(&a, "POST", "/productos", Some(json!({"nombre": "Mate", "precio_centavos": 2}))).await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert!(e["error"].as_str().unwrap().contains("ya existe"));
    let (st, _, _) = pedir(&a, "PUT", "/productos/999", Some(json!({"nombre": "z", "precio_centavos": 1}))).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn listado_con_busqueda_y_paginacion() {
    let a = nueva_app().await;
    for (i, n) in ["Mate de calabaza", "Bombilla", "Mate de vidrio", "Termo", "100%_algodón"].iter().enumerate() {
        pedir(&a, "POST", "/productos", Some(json!({"nombre": n, "precio_centavos": i * 100}))).await;
    }
    let (_, _, v) = pedir(&a, "GET", "/productos?q=mate", None).await;
    assert_eq!(v.as_array().unwrap().len(), 2); // LIKE de SQLite no distingue mayúsculas ASCII
    let (_, _, v) = pedir(&a, "GET", "/productos?limite=2&desplazamiento=1", None).await;
    let nombres: Vec<&str> = v.as_array().unwrap().iter().map(|p| p["nombre"].as_str().unwrap()).collect();
    assert_eq!(nombres, vec!["Bombilla", "Mate de vidrio"]);
    // "%" del usuario se toma literal, no como comodín.
    let (_, _, v) = pedir(&a, "GET", "/productos?q=%25_", None).await;
    assert_eq!(v.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn salud() {
    let a = nueva_app().await;
    let r = a.oneshot(Request::get("/salud").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
}
