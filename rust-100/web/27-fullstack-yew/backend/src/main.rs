//! `cargo run -p backend` → http://localhost:3000. Vars: PUERTO, DIR_PKG (frontend/pkg).

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let pkg = std::env::var("DIR_PKG").unwrap_or_else(|_| "frontend/pkg".into());
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("Full-stack en http://{} (WASM desde {pkg})", l.local_addr()?);
    axum::serve(l, backend::app(Default::default(), &pkg)).await?;
    Ok(())
}
