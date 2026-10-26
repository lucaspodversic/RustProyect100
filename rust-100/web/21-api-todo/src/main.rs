//! Servidor To-Do: DATABASE_URL (sqlite://todo.db), PUERTO (3000).

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://todo.db".into());
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let db = api_todo::crear_pool(&url).await?;
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("To-Do API en http://{}", l.local_addr()?);
    axum::serve(l, api_todo::app(db)).await?;
    Ok(())
}
