//! Tests de subida/descarga con multipart armado a mano y directorio temporal.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use subida_archivos::{app, sanear_nombre, Config};
use tower::ServiceExt;

const LIMITE: &str = "XLIMITEX";

/// Arma un cuerpo multipart/form-data con un único campo.
fn multipart(campo: &str, nombre: &str, datos: &[u8]) -> Request<Body> {
    let mut b = format!("--{LIMITE}\r\nContent-Disposition: form-data; name=\"{campo}\"; filename=\"{nombre}\"\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes();
    b.extend_from_slice(datos);
    b.extend_from_slice(format!("\r\n--{LIMITE}--\r\n").as_bytes());
    Request::post("/archivos").header("content-type", format!("multipart/form-data; boundary={LIMITE}")).body(Body::from(b)).unwrap()
}

async fn enviar(a: &axum::Router, r: Request<Body>) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let res = a.clone().oneshot(r).await.unwrap();
    let (p, b) = res.into_parts();
    (p.status, p.headers, b.collect().await.unwrap().to_bytes().to_vec())
}

fn nueva(max: usize) -> (axum::Router, tempfile::TempDir) {
    let d = tempfile::tempdir().unwrap();
    let mut c = Config::new(d.path().to_path_buf());
    c.max_bytes = max;
    (app(c), d)
}

#[test]
fn nombres_saneados() {
    assert_eq!(sanear_nombre("../../etc/passwd"), "passwd");
    assert_eq!(sanear_nombre("C:\\x\\a\"b;.txt"), "ab.txt");
    assert_eq!(sanear_nombre(".bashrc"), "bashrc");
    assert_eq!(sanear_nombre(""), "archivo");
}

#[tokio::test]
async fn subir_listar_descargar_borrar() {
    let (a, _d) = nueva(1024);
    let png = b"\x89PNG\r\n\x1a\nresto-de-la-imagen";
    let (st, _, b) = enviar(&a, multipart("archivo", "../foto.png", png)).await;
    assert_eq!(st, StatusCode::CREATED);
    let m: serde_json::Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(m["mime"], "image/png");
    assert_eq!(m["nombre"], "foto.png");
    let id = m["id"].as_str().unwrap().to_string();

    // Subir lo mismo de nuevo da el mismo id (deduplicación).
    let (_, _, b2) = enviar(&a, multipart("archivo", "otra.png", png)).await;
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&b2).unwrap()["id"], id.as_str());

    let (_, _, l) = enviar(&a, Request::get("/archivos").body(Body::empty()).unwrap()).await;
    assert_eq!(serde_json::from_slice::<Vec<serde_json::Value>>(&l).unwrap().len(), 1);

    let (st, h, cuerpo) = enviar(&a, Request::get(format!("/archivos/{id}")).body(Body::empty()).unwrap()).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(cuerpo, png);
    assert_eq!(h["content-type"], "image/png");
    assert_eq!(h["x-content-type-options"], "nosniff");
    assert!(h["content-disposition"].to_str().unwrap().starts_with("attachment"));

    assert_eq!(enviar(&a, Request::delete(format!("/archivos/{id}")).body(Body::empty()).unwrap()).await.0, StatusCode::NO_CONTENT);
    assert_eq!(enviar(&a, Request::get(format!("/archivos/{id}")).body(Body::empty()).unwrap()).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn limites_de_tamano_y_tipo() {
    let (a, d) = nueva(100);
    let grande = vec![b'a'; 101];
    assert_eq!(enviar(&a, multipart("archivo", "g.txt", &grande)).await.0, StatusCode::PAYLOAD_TOO_LARGE);
    // Un body MUY grande lo corta DefaultBodyLimit.
    let enorme = vec![b'a'; 100_000];
    assert_eq!(enviar(&a, multipart("archivo", "e.txt", &enorme)).await.0, StatusCode::PAYLOAD_TOO_LARGE);
    // Ejecutable disfrazado de imagen → 415.
    assert_eq!(enviar(&a, multipart("archivo", "gatito.png", b"MZ\x90\x00\x03")).await.0, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(enviar(&a, multipart("otro", "x.txt", b"hola")).await.0, StatusCode::BAD_REQUEST);
    // Nada quedó en disco.
    assert_eq!(std::fs::read_dir(d.path()).map(|r| r.count()).unwrap_or(0), 0);
}

#[tokio::test]
async fn ids_invalidos_no_tocan_disco() {
    let (a, _d) = nueva(100);
    for id in ["..%2F..%2Fetc%2Fpasswd", "abc", &"G".repeat(64)] {
        assert_eq!(enviar(&a, Request::get(format!("/archivos/{id}")).body(Body::empty()).unwrap()).await.0, StatusCode::NOT_FOUND);
    }
}
