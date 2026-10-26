//! Tests: ejecución directa del esquema (rápida) + un test HTTP vía Axum.

use async_graphql::{value, Request};
use futures_util::StreamExt;
use graphql_biblioteca::{app, esquema, Datos};

async fn sembrar(s: &graphql_biblioteca::Esquema) {
    let r = s
        .execute(
            r#"mutation {
                a: crearAutor(nombre: "Borges") { id }
                b: crearAutor(nombre: "Cortázar") { id }
                l1: crearLibro(datos: {titulo: "Ficciones", anio: 1944, autorId: "1"}) { id }
                l2: crearLibro(datos: {titulo: "El Aleph", anio: 1949, autorId: "1"}) { id }
                l3: crearLibro(datos: {titulo: "Rayuela", anio: 1963, autorId: "2"}) { id }
            }"#,
        )
        .await;
    assert!(r.errors.is_empty(), "{:?}", r.errors);
}

#[tokio::test]
async fn consultas_anidadas_y_filtros() {
    let s = esquema(Datos::default());
    sembrar(&s).await;
    let r = s.execute(r#"{ autores { nombre libros { titulo } } }"#).await;
    assert_eq!(
        r.data,
        value!({"autores": [
            {"nombre": "Borges", "libros": [{"titulo": "Ficciones"}, {"titulo": "El Aleph"}]},
            {"nombre": "Cortázar", "libros": [{"titulo": "Rayuela"}]}
        ]})
    );
    let r = s.execute(r#"{ libros(filtro: {anioDesde: 1945}, limite: 1) { total hayMas items { titulo autor { nombre } } } }"#).await;
    assert_eq!(r.data, value!({"libros": {"total": 2, "hayMas": true, "items": [{"titulo": "El Aleph", "autor": {"nombre": "Borges"}}]}}));
    let r = s.execute(r#"{ libros(filtro: {texto: "RAYU"}) { total } }"#).await;
    assert_eq!(r.data, value!({"libros": {"total": 1}}));
}

#[tokio::test]
async fn errores_con_codigo_y_prestamos() {
    let s = esquema(Datos::default());
    sembrar(&s).await;
    let codigo = |r: &async_graphql::Response| r.errors[0].extensions.as_ref().and_then(|e| e.get("codigo").cloned());

    let r = s.execute(r#"mutation { crearLibro(datos: {titulo: "X", anio: 2000, autorId: "99"}) { id } }"#).await;
    assert_eq!(codigo(&r), Some(value!("NO_ENCONTRADO")));
    assert!(s.execute(r#"mutation { prestar(id: "1") { disponible } }"#).await.errors.is_empty());
    let r = s.execute(r#"mutation { prestar(id: "1") { disponible } }"#).await;
    assert_eq!(codigo(&r), Some(value!("NO_DISPONIBLE")));
    let r = s.execute(r#"{ libros(filtro: {soloDisponibles: true}) { total } }"#).await;
    assert_eq!(r.data, value!({"libros": {"total": 2}}));
    let r = s.execute(r#"{ libros(limite: 1000) { total } }"#).await;
    assert_eq!(codigo(&r), Some(value!("INVALIDO")));
    // Variables tipadas.
    let r = s.execute(Request::new("query($id: ID!) { libro(id: $id) { titulo } }").variables(async_graphql::Variables::from_json(serde_json::json!({"id": "3"})))).await;
    assert_eq!(r.data, value!({"libro": {"titulo": "Rayuela"}}));
}

#[tokio::test]
async fn limite_de_profundidad() {
    let s = esquema(Datos::default());
    let r = s.execute(r#"{ autores { libros { autor { libros { autor { libros { autor { nombre } } } } } } } }"#).await;
    assert!(r.errors[0].message.contains("nested too deep"), "{:?}", r.errors);
}

#[tokio::test]
async fn suscripcion_recibe_libros_nuevos() {
    let s = esquema(Datos::default());
    let mut flujo = s.execute_stream("subscription { libroCreado { titulo } }");
    // Disparamos la creación en paralelo; el primer poll del stream registra la suscripción.
    let s2 = s.clone();
    let tarea = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        s2.execute(r#"mutation { crearAutor(nombre: "A") { id } }"#).await;
        s2.execute(r#"mutation { crearLibro(datos: {titulo: "Nuevo", anio: 2026, autorId: "1"}) { id } }"#).await;
    });
    let r = tokio::time::timeout(std::time::Duration::from_secs(3), flujo.next()).await.unwrap().unwrap();
    assert_eq!(r.data, value!({"libroCreado": {"titulo": "Nuevo"}}));
    tarea.await.unwrap();
}

#[tokio::test]
async fn http_post_graphql() {
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let a = app(esquema(Datos::default()));
    let req = axum::http::Request::post("/graphql")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"query":"mutation { crearAutor(nombre: \"Ana\") { id nombre } }"}"#))
        .unwrap();
    let r = a.oneshot(req).await.unwrap();
    assert_eq!(r.status(), 200);
    let v: serde_json::Value = serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(v["data"]["crearAutor"]["nombre"], "Ana");
}
