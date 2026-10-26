//! `cargo run` → http://localhost:3000. Probar: `for i in $(seq 15); do curl -si localhost:3000/ | head -1; done`

use rate_limiter::balde::Config;
use rate_limiter::{app, por_ip, LimiteLayer};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let general = LimiteLayer::new(Config { rafaga: 10.0, por_segundo: 5.0, max_claves: 100_000 }, por_ip());
    let login = LimiteLayer::new(Config { rafaga: 3.0, por_segundo: 0.1, max_claves: 100_000 }, por_ip());
    // Tarea de mantenimiento: purga baldes llenos cada minuto.
    for capa in [general.clone(), login.clone()] {
        tokio::spawn(async move {
            let mut t = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                t.tick().await;
                capa.purgar();
            }
        });
    }
    let l = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Rate limiter en http://{}", l.local_addr()?);
    // ConnectInfo es lo que permite a `por_ip` ver la IP del cliente.
    axum::serve(l, app(general, login).into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}
