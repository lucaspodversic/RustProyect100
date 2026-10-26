//! Servidor del blog: DATABASE_URL (sqlite://blog.db), PUERTO (3000).

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://blog.db".into());
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let db = backend_blog::repo::crear_pool(&url).await?;
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("Blog en http://{}", l.local_addr()?);
    axum::serve(l, backend_blog::app(db)).await?;
    Ok(())
}
