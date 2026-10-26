//! Servidor del acortador. Variables: DATABASE_URL, PUERTO, BASE_PUBLICA.

use acortador_urls::{app, crear_pool, Estado};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://enlaces.db".into());
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let base = std::env::var("BASE_PUBLICA").unwrap_or_else(|_| format!("http://localhost:{puerto}"));
    let estado = Estado { db: crear_pool(&url).await?, base: base.into() };
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("Acortador en http://{}", l.local_addr()?);
    axum::serve(l, app(estado)).await?;
    Ok(())
}
