//! Tests: repositorio directo + API HTTP vía oneshot.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use backend_blog::repo::{self, slugificar, DatosPost};
use backend_blog::app;
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

#[test]
fn slugs() {
    assert_eq!(slugificar("¡Hola, Año Nuevo!"), "hola-ano-nuevo");
    assert_eq!(slugificar("  Rust   &  Axum 0.8 "), "rust-axum-0-8");
    assert_eq!(slugificar("???"), "");
}

#[tokio::test]
async fn repo_slugs_unicos_y_etiquetas() {
    let db = repo::crear_pool("sqlite::memory:").await.unwrap();
    let d = DatosPost { titulo: "Hola".into(), cuerpo: "x".into(), publicado: true, etiquetas: vec!["Rust".into(), " rust ".into(), "web".into()] };
    let a = repo::crear_post(&db, &d).await.unwrap();
    let b = repo::crear_post(&db, &d).await.unwrap();
    assert_eq!((a.slug.as_str(), b.slug.as_str()), ("hola", "hola-2"));
    let full = repo::obtener(&db, "hola").await.unwrap().unwrap();
    assert_eq!(full.etiquetas, vec!["rust", "web"]); // normalizadas y sin duplicar
    assert_eq!(repo::nube_etiquetas(&db).await.unwrap(), vec![("rust".into(), 2), ("web".into(), 2)]);
}

#[tokio::test]
async fn api_flujo_completo() {
    let a = app(repo::crear_pool("sqlite::memory:").await.unwrap());
    let (st, p) = pedir(&a, "POST", "/posts", Some(json!({"titulo": "Mi primer post", "cuerpo": "Hola mundo", "publicado": true, "etiquetas": ["rust", "intro"]}))).await;
    assert_eq!(st, StatusCode::CREATED);
    assert_eq!(p["slug"], "mi-primer-post");
    pedir(&a, "POST", "/posts", Some(json!({"titulo": "Borrador", "cuerpo": "wip", "etiquetas": ["rust"]}))).await;

    // Listado: borradores ocultos salvo que se pidan.
    let (_, v) = pedir(&a, "GET", "/posts", None).await;
    assert_eq!(v.as_array().unwrap().len(), 1);
    let (_, v) = pedir(&a, "GET", "/posts?borradores=true&etiqueta=RUST", None).await;
    assert_eq!(v.as_array().unwrap().len(), 2);
    let (_, v) = pedir(&a, "GET", "/posts?etiqueta=intro", None).await;
    assert_eq!(v[0]["slug"], "mi-primer-post");

    // Comentarios: en publicados sí, en borradores no.
    let (st, _) = pedir(&a, "POST", "/posts/mi-primer-post/comentarios", Some(json!({"autor": "Ana", "texto": "¡Bien!"}))).await;
    assert_eq!(st, StatusCode::CREATED);
    let (st, _) = pedir(&a, "POST", "/posts/borrador/comentarios", Some(json!({"autor": "Ana", "texto": "x"}))).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (_, d) = pedir(&a, "GET", "/posts/mi-primer-post", None).await;
    assert_eq!(d["comentarios"][0]["autor"], "Ana");
    assert_eq!(d["etiquetas"], json!(["intro", "rust"]));

    // Actualizar reemplaza etiquetas pero mantiene slug.
    let (st, u) = pedir(&a, "PUT", "/posts/mi-primer-post", Some(json!({"titulo": "Título nuevo", "cuerpo": "c", "publicado": true, "etiquetas": ["axum"]}))).await;
    assert_eq!((st, u["slug"].as_str()), (StatusCode::OK, Some("mi-primer-post")));
    let (_, e) = pedir(&a, "GET", "/etiquetas", None).await;
    assert_eq!(e, json!([{"nombre": "axum", "posts": 1}, {"nombre": "rust", "posts": 1}]));

    // Borrar elimina comentarios en cascada.
    assert_eq!(pedir(&a, "DELETE", "/posts/mi-primer-post", None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(pedir(&a, "GET", "/posts/mi-primer-post", None).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cascada_real_en_la_base() {
    let db = repo::crear_pool("sqlite::memory:").await.unwrap();
    let d = DatosPost { titulo: "P".into(), cuerpo: "x".into(), publicado: true, etiquetas: vec![] };
    repo::crear_post(&db, &d).await.unwrap();
    repo::comentar(&db, "p", "a", "b").await.unwrap().unwrap();
    repo::borrar_post(&db, "p").await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comentarios").fetch_one(&db).await.unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn validaciones() {
    let a = app(repo::crear_pool("sqlite::memory:").await.unwrap());
    assert_eq!(pedir(&a, "POST", "/posts", Some(json!({"titulo": " ", "cuerpo": "x"}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let muchas: Vec<String> = (0..11).map(|i| format!("e{i}")).collect();
    assert_eq!(pedir(&a, "POST", "/posts", Some(json!({"titulo": "t", "cuerpo": "x", "etiquetas": muchas}))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
}
