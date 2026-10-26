//! Tests de integración: SQLite en memoria y "ahora" fijo (2026-10-26 08:00).

use api_reservas::{agenda::parse_fecha_hora, app, crear_pool, Estado};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

async fn servidor() -> axum::Router {
    let mut e = Estado::nuevo(crear_pool("sqlite::memory:").await.unwrap());
    let ahora = parse_fecha_hora("2026-10-26T08:00").unwrap();
    e.ahora = Arc::new(move || ahora);
    app(e)
}

async fn pedir(a: &axum::Router, m: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut b = Request::builder().method(m).uri(uri);
    if body.is_some() {
        b = b.header("content-type", "application/json");
    }
    let r = a.clone().oneshot(b.body(body.map(|v| Body::from(v.to_string())).unwrap_or_default()).unwrap()).await.unwrap();
    let s = r.status();
    let by = r.into_body().collect().await.unwrap().to_bytes();
    (s, serde_json::from_slice(&by).unwrap_or(Value::Null))
}

async fn con_sala(a: &axum::Router) {
    let (s, _) = pedir(a, "POST", "/recursos", Some(json!({"nombre":"Sala A","apertura":"09:00","cierre":"18:00","granularidad_min":30}))).await;
    assert_eq!(s, StatusCode::CREATED);
}

fn reserva(ini: &str, fin: &str, quien: &str) -> Value {
    json!({"recurso_id":1,"inicio":format!("2026-10-27T{ini}"),"fin":format!("2026-10-27T{fin}"),"titular":quien})
}

#[tokio::test]
async fn conflictos() {
    let a = servidor().await;
    con_sala(&a).await;
    assert_eq!(pedir(&a, "POST", "/reservas", Some(reserva("10:00", "11:00", "Ana"))).await.0, StatusCode::CREATED);
    // Bordes que se tocan NO chocan.
    assert_eq!(pedir(&a, "POST", "/reservas", Some(reserva("11:00", "12:00", "Beto"))).await.0, StatusCode::CREATED);
    assert_eq!(pedir(&a, "POST", "/reservas", Some(reserva("09:00", "10:00", "Caro"))).await.0, StatusCode::CREATED);
    // Pisa dos reservas → 409 con ambas listadas.
    let (s, v) = pedir(&a, "POST", "/reservas", Some(reserva("10:30", "11:30", "Dani"))).await;
    assert_eq!(s, StatusCode::CONFLICT);
    let quienes: Vec<&str> = v["conflictos"].as_array().unwrap().iter().map(|c| c["titular"].as_str().unwrap()).collect();
    assert_eq!(quienes, ["Ana", "Beto"]);
    // Contenida dentro de otra → 409.
    assert_eq!(pedir(&a, "POST", "/reservas", Some(reserva("10:00", "10:30", "Eva"))).await.0, StatusCode::CONFLICT);
    // Cancelar libera el lugar; cancelar dos veces → 404.
    assert_eq!(pedir(&a, "DELETE", "/reservas/1", None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(pedir(&a, "DELETE", "/reservas/1", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(pedir(&a, "POST", "/reservas", Some(reserva("10:00", "10:30", "Eva"))).await.0, StatusCode::CREATED);

    let (_, lista) = pedir(&a, "GET", "/reservas?recurso_id=1&fecha=2026-10-27", None).await;
    assert_eq!(lista.as_array().unwrap().len(), 3);
    let (_, otro_dia) = pedir(&a, "GET", "/reservas?fecha=2026-10-28", None).await;
    assert_eq!(otro_dia, json!([]));
}

#[tokio::test]
async fn reservas_simultaneas_solo_gana_una() {
    let a = servidor().await;
    con_sala(&a).await;
    // 20 pedidos concurrentes por el mismo horario.
    let tareas: Vec<_> = (0..20).map(|i| {
        let a = a.clone();
        tokio::spawn(async move { pedir(&a, "POST", "/reservas", Some(reserva("14:00", "15:00", &format!("p{i}")))).await.0 })
    }).collect();
    let mut creadas = 0;
    for t in tareas {
        if t.await.unwrap() == StatusCode::CREATED {
            creadas += 1;
        }
    }
    assert_eq!(creadas, 1);
}

#[tokio::test]
async fn disponibilidad() {
    let a = servidor().await;
    con_sala(&a).await;
    pedir(&a, "POST", "/reservas", Some(reserva("10:00", "11:00", "Ana"))).await;
    pedir(&a, "POST", "/reservas", Some(reserva("12:00", "17:00", "Beto"))).await;
    let (s, v) = pedir(&a, "GET", "/recursos/1/disponibilidad?fecha=2026-10-27&duracion=60", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["libres"], json!([
        {"inicio":"2026-10-27T09:00","fin":"2026-10-27T10:00"},
        {"inicio":"2026-10-27T11:00","fin":"2026-10-27T12:00"},
        {"inicio":"2026-10-27T17:00","fin":"2026-10-27T18:00"}]));
    assert_eq!(v["inicios_posibles"], json!(["2026-10-27T09:00","2026-10-27T11:00","2026-10-27T17:00"]));
    // 90 minutos no entran en ningún hueco.
    let (_, v) = pedir(&a, "GET", "/recursos/1/disponibilidad?fecha=2026-10-27&duracion=90", None).await;
    assert_eq!(v["inicios_posibles"], json!([]));
    // Día pasado → sin huecos; recurso inexistente → 404; duración desalineada → 422.
    let (_, v) = pedir(&a, "GET", "/recursos/1/disponibilidad?fecha=2026-10-25", None).await;
    assert_eq!(v["libres"], json!([]));
    assert_eq!(pedir(&a, "GET", "/recursos/9/disponibilidad?fecha=2026-10-27", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(pedir(&a, "GET", "/recursos/1/disponibilidad?fecha=2026-10-27&duracion=45", None).await.0, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn validaciones() {
    let a = servidor().await;
    con_sala(&a).await;
    let casos = [
        (reserva("11:00", "10:00", "x"), "posterior"),
        (reserva("08:00", "09:00", "x"), "horario"),
        (reserva("17:30", "18:30", "x"), "horario"),
        (reserva("10:15", "11:00", "x"), "alineados"),
        (reserva("09:00", "17:30", "x"), "8 horas"),
        (reserva("10:00", "11:00", "  "), "titular"),
        (json!({"recurso_id":1,"inicio":"2026-10-20T10:00","fin":"2026-10-20T11:00","titular":"x"}), "pasado"),
        (json!({"recurso_id":1,"inicio":"27/10/2026 10:00","fin":"2026-10-27T11:00","titular":"x"}), "YYYY"),
    ];
    for (cuerpo, esperado) in casos {
        let (s, v) = pedir(&a, "POST", "/reservas", Some(cuerpo)).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
        assert!(v["error"].as_str().unwrap().contains(esperado), "{v} debería mencionar {esperado}");
    }
    let (s, _) = pedir(&a, "POST", "/recursos", Some(json!({"nombre":"Mala","apertura":"09:10","cierre":"18:00","granularidad_min":30}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, r) = pedir(&a, "GET", "/recursos", None).await;
    assert_eq!(r[0]["apertura"], "09:00");
}
