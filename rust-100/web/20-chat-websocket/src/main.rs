//! Servidor de chat: PUERTO (3000). Abrí http://localhost:3000 en dos pestañas.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("Chat en http://{}", l.local_addr()?);
    axum::serve(l, chat_websocket::app(chat_websocket::salas::Salas::new(50))).await?;
    Ok(())
}
