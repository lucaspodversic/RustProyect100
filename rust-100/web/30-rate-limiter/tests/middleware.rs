//! Tests del middleware con reloj falso, y uno con servidor TCP real.

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use rate_limiter::balde::Config;
use rate_limiter::{app, por_header, por_ip, LimiteLayer};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

fn capa(rafaga: f64, por_segundo: f64, reloj: &Arc<AtomicU64>) -> LimiteLayer {
    let r = reloj.clone();
    LimiteLayer::new(Config { rafaga, por_segundo, max_claves: 1000 }, por_ip())
        .con_reloj(Arc::new(move || r.load(Ordering::SeqCst)))
}

fn req(metodo: &str, uri: &str, ip: &str) -> Request<Body> {
    let mut r = Request::builder().method(metodo).uri(uri).body(Body::empty()).unwrap();
    let addr: SocketAddr = format!("{ip}:5555").parse().unwrap();
    r.extensions_mut().insert(ConnectInfo(addr));
    r
}

#[tokio::test]
async fn limita_por_ip_con_cabeceras() {
    let reloj = Arc::new(AtomicU64::new(0));
    let a = app(capa(3.0, 1.0, &reloj), capa(100.0, 1.0, &reloj));
    for restantes in ["2", "1", "0"] {
        let r = a.clone().oneshot(req("GET", "/", "10.0.0.1")).await.unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers()["x-ratelimit-limit"], "3");
        assert_eq!(r.headers()["x-ratelimit-remaining"], restantes);
    }
    let r = a.clone().oneshot(req("GET", "/", "10.0.0.1")).await.unwrap();
    assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(r.headers()["retry-after"], "1");
    // Otra IP no se ve afectada.
    assert_eq!(a.clone().oneshot(req("GET", "/", "10.0.0.2")).await.unwrap().status(), StatusCode::OK);
    // El límite es por cliente, no por ruta: /salud comparte el balde general.
    assert_eq!(a.clone().oneshot(req("GET", "/salud", "10.0.0.1")).await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
    // Tras 1 s vuelve 1 ficha.
    reloj.store(1000, Ordering::SeqCst);
    assert_eq!(a.clone().oneshot(req("GET", "/", "10.0.0.1")).await.unwrap().status(), StatusCode::OK);
    assert_eq!(a.clone().oneshot(req("GET", "/", "10.0.0.1")).await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn login_tiene_limite_mas_estricto() {
    let reloj = Arc::new(AtomicU64::new(0));
    let login = capa(2.0, 0.1, &reloj); // 1 intento cada 10 s
    let a = app(capa(100.0, 10.0, &reloj), login.clone());
    for _ in 0..2 {
        assert_eq!(a.clone().oneshot(req("POST", "/login", "1.2.3.4")).await.unwrap().status(), StatusCode::OK);
    }
    let r = a.clone().oneshot(req("POST", "/login", "1.2.3.4")).await.unwrap();
    assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(r.headers()["retry-after"], "10");
    // El resto del sitio sigue disponible para esa IP.
    assert_eq!(a.clone().oneshot(req("GET", "/", "1.2.3.4")).await.unwrap().status(), StatusCode::OK);
    // Purga: tras recargarse por completo, el balde se elimina.
    assert_eq!(login.claves(), 1);
    reloj.store(60_000, Ordering::SeqCst);
    login.purgar();
    assert_eq!(login.claves(), 0);
}

#[tokio::test]
async fn clave_por_header_y_anonimos() {
    let reloj = Arc::new(AtomicU64::new(0));
    let r = reloj.clone();
    let capa = LimiteLayer::new(Config { rafaga: 1.0, por_segundo: 1.0, max_claves: 10 }, por_header("x-api-key"))
        .con_reloj(Arc::new(move || r.load(Ordering::SeqCst)));
    let svc = axum::Router::new().route("/", axum::routing::get(|| async { "ok" })).layer(capa);
    let con_clave = |k: &str| Request::builder().uri("/").header("x-api-key", k).body(Body::empty()).unwrap();
    assert_eq!(svc.clone().oneshot(con_clave("A")).await.unwrap().status(), StatusCode::OK);
    assert_eq!(svc.clone().oneshot(con_clave("A")).await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(svc.clone().oneshot(con_clave("B")).await.unwrap().status(), StatusCode::OK);
    // Sin header → todos comparten el balde "anonimo".
    let sin = || Request::builder().uri("/").body(Body::empty()).unwrap();
    assert_eq!(svc.clone().oneshot(sin()).await.unwrap().status(), StatusCode::OK);
    assert_eq!(svc.clone().oneshot(sin()).await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn servidor_real_con_connect_info() {
    let general = LimiteLayer::new(Config { rafaga: 2.0, por_segundo: 0.001, max_claves: 10 }, por_ip());
    let login = LimiteLayer::new(Config { rafaga: 2.0, por_segundo: 0.001, max_claves: 10 }, por_ip());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(l, app(general, login).into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
    });
    let mut codigos = Vec::new();
    for _ in 0..3 {
        // HTTP/1.1 crudo para no sumar un cliente HTTP como dependencia.
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
        s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").await.unwrap();
        let mut buf = String::new();
        s.read_to_string(&mut buf).await.unwrap();
        codigos.push(buf.split_whitespace().nth(1).unwrap().to_string());
    }
    assert_eq!(codigos, ["200", "200", "429"]);
}
