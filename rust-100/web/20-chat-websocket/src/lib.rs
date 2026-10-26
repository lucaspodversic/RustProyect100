//! # Chat WebSocket con salas e historial
//!
//! - `GET /ws/{sala}?nombre=ana` → upgrade a WebSocket.
//!   - Al conectar: recibe `{"tipo":"historial", ...}` y se anuncia `entro`.
//!   - Texto enviado por el cliente → se difunde como `{"tipo":"mensaje", ...}`.
//!   - Al cerrar → `salio`.
//! - `GET /salas` → salas activas y miembros.
//! - `GET /` → página HTML mínima de prueba (`static/index.html`, embebida).
//!
//! ## Concurrencia
//! Cada conexión se divide en dos tareas: una lee del socket y publica en la
//! sala; otra escucha el `broadcast` y escribe al socket. Cuando cualquiera
//! termina, se aborta la otra (`tokio::select!`).

pub mod salas;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use salas::{Evento, Salas};
use serde::Deserialize;

/// Límite de longitud por mensaje (evita abuso de memoria).
pub const MAX_TEXTO: usize = 1000;

pub fn app(s: Salas) -> Router {
    Router::new()
        .route("/", get(|| async { Html(include_str!("../static/index.html")) }))
        .route("/salas", get(listar))
        .route("/ws/{sala}", get(ws))
        .with_state(s)
}

async fn listar(State(s): State<Salas>) -> Json<serde_json::Value> {
    Json(serde_json::json!(s.listar().into_iter().map(|(n, m)| serde_json::json!({"sala": n, "miembros": m})).collect::<Vec<_>>()))
}

#[derive(Deserialize)]
struct Params {
    nombre: String,
}

fn nombre_valido(n: &str) -> bool {
    (1..=24).contains(&n.chars().count()) && n.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

async fn ws(ws: WebSocketUpgrade, Path(sala): Path<String>, Query(p): Query<Params>, State(s): State<Salas>) -> Response {
    if !nombre_valido(&p.nombre) || !nombre_valido(&sala) {
        return (axum::http::StatusCode::BAD_REQUEST, "nombre/sala inválidos: 1-24 alfanuméricos, _ o -").into_response();
    }
    // max_message_size: el servidor corta frames gigantes antes de que lleguen a nuestro código.
    ws.max_message_size(16 * 1024).on_upgrade(move |socket| atender(socket, s, sala, p.nombre))
}

async fn atender(socket: WebSocket, s: Salas, sala: String, nombre: String) {
    let (mut tx_ws, mut rx_ws) = socket.split();
    let enviar = |e: &Evento| Message::Text(serde_json::to_string(e).expect("json").into());

    let union = match s.unir(&sala, &nombre) {
        Ok(u) => u,
        Err(motivo) => {
            let _ = tx_ws.send(enviar(&Evento::Error { motivo })).await;
            let _ = tx_ws.close().await;
            return;
        }
    };
    if tx_ws.send(enviar(&Evento::Historial { mensajes: union.historial })).await.is_err() {
        s.salir(&sala, &nombre);
        return;
    }

    let mut rx = union.rx;
    // Tarea de escritura: broadcast → socket.
    let mut escritor = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(e) => {
                    if tx_ws.send(enviar(&e)).await.is_err() {
                        break;
                    }
                }
                // El cliente es lento y se perdieron mensajes: seguimos con los nuevos.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });
    // Tarea de lectura: socket → sala.
    let (s2, sala2, nombre2) = (s.clone(), sala.clone(), nombre.clone());
    let mut lector = tokio::spawn(async move {
        while let Some(Ok(m)) = rx_ws.next().await {
            match m {
                Message::Text(t) => {
                    let t = t.trim();
                    if !t.is_empty() {
                        let recortado: String = t.chars().take(MAX_TEXTO).collect();
                        s2.publicar(&sala2, &nombre2, &recortado);
                    }
                }
                Message::Close(_) => break,
                _ => {} // ping/pong los maneja axum; binarios se ignoran
            }
        }
    });
    tokio::select! {
        _ = &mut escritor => lector.abort(),
        _ = &mut lector => escritor.abort(),
    }
    s.salir(&sala, &nombre);
}
