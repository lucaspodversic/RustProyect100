//! Tests del backend: API + página SSR que integra el frontend Yew.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn pedir(a: &axum::Router, m: &str, uri: &str, body: Option<&str>) -> (StatusCode, String) {
    let mut b = Request::builder().method(m).uri(uri);
    if body.is_some() {
        b = b.header("content-type", "application/json");
    }
    let r = a.clone().oneshot(b.body(body.map(|s| Body::from(s.to_string())).unwrap_or_default()).unwrap()).await.unwrap();
    let s = r.status();
    (s, String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap())
}

#[tokio::test]
async fn api_y_ssr_comparten_estado() {
    let a = backend::app(Default::default(), "no-existe");
    let (s, _) = pedir(&a, "POST", "/api/notas", Some(r#"{"texto":"  comprar yerba  "}"#)).await;
    assert_eq!(s, StatusCode::CREATED);
    pedir(&a, "POST", "/api/notas", Some(r#"{"texto":"</script><b>x</b>"}"#)).await;
    let (s, n) = pedir(&a, "POST", "/api/notas/1/alternar", None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(n.contains(r#""hecha":true"#));

    // Validación compartida: el backend rechaza lo mismo que el frontend.
    assert_eq!(pedir(&a, "POST", "/api/notas", Some(r#"{"texto":"   "}"#)).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    let (s, html) = pedir(&a, "GET", "/", None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(html.contains("comprar yerba"), "SSR debe incluir las notas");
    assert!(html.contains("1 pendiente(s)"));
    // La nota maliciosa no rompe el <script> de datos ni el HTML.
    assert_eq!(html.matches("</script>").count(), 2, "{html}");
    assert!(html.contains(r#"\u003c/script>"#));

    assert_eq!(pedir(&a, "DELETE", "/api/notas/1", None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(pedir(&a, "DELETE", "/api/notas/1", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(pedir(&a, "GET", "/pkg/frontend.js", None).await.0, StatusCode::NOT_FOUND);
}
