//! Tests: (1) API con proveedor mock y reloj controlado; (2) cliente OpenMeteo
//! real contra un servidor HTTP falso local (sin salir a internet).

use api_clima::cache::{Cache, Reloj};
use api_clima::proveedor::{Clima, ErrorProveedor, OpenMeteo, Proveedor};
use api_clima::{app, Estado};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tower::ServiceExt;

/// Mock: cuenta llamadas y puede simular caída.
#[derive(Default)]
struct Mock {
    llamadas: AtomicU64,
    caido: AtomicBool,
}
/// Newtype: la regla de huérfanos impide `impl Proveedor for Arc<Mock>` desde
/// otro crate, así que envolvemos el Arc para compartir el mock con el test.
struct MockCompartido(Arc<Mock>);
impl Proveedor for MockCompartido {
    async fn consultar(&self, lat: f64, _lon: f64) -> Result<Clima, ErrorProveedor> {
        let m = &self.0;
        m.llamadas.fetch_add(1, Ordering::SeqCst);
        if m.caido.load(Ordering::SeqCst) {
            return Err(ErrorProveedor::NoDisponible("simulado".into()));
        }
        Ok(Clima { temperatura_c: lat, viento_kmh: 10.0, codigo: 0, descripcion: "despejado".into() })
    }
}

/// Reloj que el test avanza a mano.
#[derive(Clone)]
struct RelojFalso(Arc<Mutex<Instant>>);
impl Reloj for RelojFalso {
    fn ahora(&self) -> Instant {
        *self.0.lock().unwrap()
    }
}

async fn get(a: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let r = a.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    let s = r.status();
    (s, serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap())
}

#[tokio::test]
async fn cache_ttl_y_stale_if_error() {
    let mock = Arc::new(Mock::default());
    let reloj = RelojFalso(Arc::new(Mutex::new(Instant::now())));
    let a = app(Estado::new(MockCompartido(mock.clone()), Cache::new(Duration::from_secs(60), 100), reloj.clone()));

    let (s, v) = get(&a, "/clima?lat=-34.60&lon=-58.38").await;
    assert_eq!((s, v["origen"].as_str()), (StatusCode::OK, Some("proveedor")));
    // Coordenadas casi iguales → misma entrada.
    let (_, v) = get(&a, "/clima?lat=-34.601&lon=-58.381").await;
    assert_eq!(v["origen"], "cache");
    assert_eq!(mock.llamadas.load(Ordering::SeqCst), 1);

    // Pasa el TTL y el proveedor se cae → cache vencida.
    *reloj.0.lock().unwrap() += Duration::from_secs(61);
    mock.caido.store(true, Ordering::SeqCst);
    let (s, v) = get(&a, "/clima?lat=-34.6&lon=-58.38").await;
    assert_eq!((s, v["origen"].as_str()), (StatusCode::OK, Some("cache-vencida")));
    // Sin nada en caché y proveedor caído → 503.
    assert_eq!(get(&a, "/clima?lat=10&lon=10").await.0, StatusCode::SERVICE_UNAVAILABLE);
    // Se recupera → refresca.
    mock.caido.store(false, Ordering::SeqCst);
    assert_eq!(get(&a, "/clima?lat=-34.6&lon=-58.38").await.1["origen"], "proveedor");

    let (_, m) = get(&a, "/metricas").await;
    assert_eq!(m["aciertos"], 1);
    assert_eq!(m["llamadas_proveedor"], 4);
}

#[tokio::test]
async fn validacion_de_coordenadas() {
    let a = app(Estado::new(MockCompartido(Arc::default()), Cache::new(Duration::from_secs(1), 1), api_clima::cache::RelojReal));
    assert_eq!(get(&a, "/clima?lat=91&lon=0").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(get(&a, "/clima?lat=0&lon=-181").await.0, StatusCode::BAD_REQUEST);
}

/// Levanta un "Open-Meteo" falso en 127.0.0.1:0.
async fn falso_open_meteo(cuerpo: &'static str, estado: u16) -> String {
    use axum::routing::get;
    let r = axum::Router::new().route(
        "/v1/forecast",
        get(move |q: axum::extract::Query<std::collections::HashMap<String, String>>| async move {
            assert!(q.contains_key("latitude") && q["current"].contains("weather_code"));
            (StatusCode::from_u16(estado).unwrap(), [("content-type", "application/json")], cuerpo)
        }),
    );
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let d = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, r).await.unwrap() });
    format!("http://{d}")
}

#[tokio::test]
async fn cliente_open_meteo_contra_servidor_falso() {
    let ok = falso_open_meteo(r#"{"latitude":-34.6,"current":{"temperature_2m":21.5,"wind_speed_10m":12.0,"weather_code":61}}"#, 200).await;
    let c = OpenMeteo::new(ok).consultar(-34.6, -58.4).await.unwrap();
    assert_eq!(c.temperatura_c, 21.5);
    assert_eq!(c.descripcion, "lluvia");

    let malo = falso_open_meteo(r#"{"otra":"cosa"}"#, 200).await;
    assert!(matches!(OpenMeteo::new(malo).consultar(0.0, 0.0).await, Err(ErrorProveedor::Formato(_))));
    let caido = falso_open_meteo("{}", 503).await;
    assert!(matches!(OpenMeteo::new(caido).consultar(0.0, 0.0).await, Err(ErrorProveedor::NoDisponible(_))));
    // Puerto cerrado → NoDisponible.
    assert!(matches!(OpenMeteo::new("http://127.0.0.1:1").consultar(0.0, 0.0).await, Err(ErrorProveedor::NoDisponible(_))));
}
