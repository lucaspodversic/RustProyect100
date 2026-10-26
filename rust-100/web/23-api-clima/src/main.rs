//! Servidor real contra Open-Meteo. Vars: OPEN_METEO_URL, TTL_SEG (600), PUERTO (3000).

use api_clima::cache::{Cache, RelojReal};
use api_clima::proveedor::OpenMeteo;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::var("OPEN_METEO_URL").unwrap_or_else(|_| "https://api.open-meteo.com".into());
    let ttl = std::env::var("TTL_SEG").ok().and_then(|v| v.parse().ok()).unwrap_or(600);
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let estado = api_clima::Estado::new(OpenMeteo::new(base), Cache::new(Duration::from_secs(ttl), 10_000), RelojReal);
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("Clima en http://{}/clima?lat=-34.6&lon=-58.4", l.local_addr()?);
    axum::serve(l, api_clima::app(estado)).await?;
    Ok(())
}
