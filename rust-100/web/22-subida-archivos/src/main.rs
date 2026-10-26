//! Servidor de archivos: DIR (./subidas), MAX_BYTES (5 MiB), PUERTO (3000).

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg = subida_archivos::Config::new(std::env::var("DIR").unwrap_or_else(|_| "subidas".into()).into());
    if let Some(m) = std::env::var("MAX_BYTES").ok().and_then(|v| v.parse().ok()) {
        cfg.max_bytes = m;
    }
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("Archivos en http://{} (dir {:?}, máx {} bytes)", l.local_addr()?, cfg.dir, cfg.max_bytes);
    axum::serve(l, subida_archivos::app(cfg)).await?;
    Ok(())
}
