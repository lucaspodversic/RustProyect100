//! Servidor: `DATABASE_URL` (por defecto `sqlite://productos.db`) y `PUERTO` (3000).

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://productos.db".into());
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let pool = api_rest_crud::crear_pool(&url).await?;
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("API escuchando en http://{}", l.local_addr()?);
    axum::serve(l, api_rest_crud::app(pool)).await?;
    Ok(())
}
