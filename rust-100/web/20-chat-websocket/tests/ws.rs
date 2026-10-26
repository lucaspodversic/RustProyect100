//! Tests de integración con un servidor real en 127.0.0.1:0 y clientes tungstenite.

use chat_websocket::{app, salas::Salas};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

async fn servidor() -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dir = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, app(Salas::new(3))).await.unwrap() });
    format!("{dir}")
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn conectar(dir: &str, sala: &str, nombre: &str) -> Ws {
    tokio_tungstenite::connect_async(format!("ws://{dir}/ws/{sala}?nombre={nombre}")).await.unwrap().0
}

/// Lee el próximo evento JSON (con timeout para que un bug no cuelgue el test).
async fn siguiente(ws: &mut Ws) -> Value {
    let m = tokio::time::timeout(Duration::from_secs(3), ws.next()).await.expect("timeout").unwrap().unwrap();
    serde_json::from_str(m.to_text().unwrap()).unwrap()
}

/// Avanza hasta un evento del tipo pedido.
async fn hasta(ws: &mut Ws, tipo: &str) -> Value {
    loop {
        let v = siguiente(ws).await;
        if v["tipo"] == tipo {
            return v;
        }
    }
}

#[tokio::test]
async fn dos_clientes_conversan_e_historial() {
    let dir = servidor().await;
    let mut ana = conectar(&dir, "rust", "ana").await;
    assert_eq!(siguiente(&mut ana).await["tipo"], "historial");
    ana.send(Message::Text("hola".into())).await.unwrap();
    let m = hasta(&mut ana, "mensaje").await; // eco a sí misma
    assert_eq!(m["texto"], "hola");

    let mut beto = conectar(&dir, "rust", "beto").await;
    let h = siguiente(&mut beto).await;
    assert_eq!(h["mensajes"][0]["texto"], "hola");
    assert_eq!(hasta(&mut ana, "entro").await["nombre"], "beto");
    beto.send(Message::Text("qué tal".into())).await.unwrap();
    let m = hasta(&mut ana, "mensaje").await;
    assert_eq!((m["de"].as_str(), m["texto"].as_str()), (Some("beto"), Some("qué tal")));

    beto.close(None).await.unwrap();
    assert_eq!(hasta(&mut ana, "salio").await["nombre"], "beto");
}

#[tokio::test]
async fn salas_aisladas_y_nombre_duplicado() {
    let dir = servidor().await;
    let mut a = conectar(&dir, "a", "ana").await;
    let mut b = conectar(&dir, "b", "beto").await;
    siguiente(&mut a).await;
    siguiente(&mut b).await;
    b.send(Message::Text("solo en b".into())).await.unwrap();
    hasta(&mut b, "mensaje").await;
    // ana no debe recibir nada de la sala b (solo su propio "entro").
    assert_eq!(siguiente(&mut a).await["tipo"], "entro");
    assert!(tokio::time::timeout(Duration::from_millis(300), a.next()).await.is_err());

    let mut dup = conectar(&dir, "a", "ANA").await;
    let e = siguiente(&mut dup).await;
    assert_eq!(e["tipo"], "error");
}

#[tokio::test]
async fn parametros_invalidos_y_listado() {
    let dir = servidor().await;
    assert!(tokio_tungstenite::connect_async(format!("ws://{dir}/ws/sala?nombre=con%20espacio")).await.is_err());
    let _c = conectar(&dir, "general", "x").await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    // GET /salas con un cliente HTTP mínimo a mano.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = tokio::net::TcpStream::connect(&dir).await.unwrap();
    s.write_all(b"GET /salas HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").await.unwrap();
    let mut r = String::new();
    s.read_to_string(&mut r).await.unwrap();
    assert!(r.contains(r#"{"miembros":1,"sala":"general"}"#), "{r}");
}
