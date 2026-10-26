//! Servidor GraphQL: abrir http://localhost:3000/graphql (GraphiQL). PUERTO (3000).

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let s = graphql_biblioteca::esquema(Default::default());
    // Exportar el SDL: `cargo run -- --sdl`
    if std::env::args().any(|a| a == "--sdl") {
        println!("{}", s.sdl());
        return Ok(());
    }
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("GraphiQL en http://{}/graphql", l.local_addr()?);
    axum::serve(l, graphql_biblioteca::app(s)).await?;
    Ok(())
}
