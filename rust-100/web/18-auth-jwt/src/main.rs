//! Servidor: JWT_SECRETO (obligatorio, ≥32 bytes), DATABASE_URL, PUERTO.

use auth_jwt::{app, crear_pool, Config, Estado};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let secreto = std::env::var("JWT_SECRETO").map_err(|_| "definí JWT_SECRETO (al menos 32 caracteres)")?;
    if secreto.len() < 32 {
        return Err("JWT_SECRETO demasiado corto (mínimo 32)".into());
    }
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://usuarios.db".into());
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let e = Estado { db: crear_pool(&url).await?, cfg: Arc::new(Config::produccion(secreto.into_bytes())) };
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("Auth en http://{}", l.local_addr()?);
    axum::serve(l, app(e)).await?;
    Ok(())
}
