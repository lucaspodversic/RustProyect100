//! `cargo run` → http://localhost:3000. Vars: DATABASE_URL (sqlite://reservas.db), PUERTO.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://reservas.db".into());
    let e = api_reservas::Estado::nuevo(api_reservas::crear_pool(&url).await?);
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("API de reservas en http://{}", l.local_addr()?);
    axum::serve(l, api_reservas::app(e)).await?;
    Ok(())
}
