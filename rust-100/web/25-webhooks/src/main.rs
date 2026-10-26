//! Servidor de webhooks. Configuración por variables de entorno:
//! - `FUENTES="github=secreto1,pagos=secreto2"`
//! - `DESTINOS="github=http://localhost:9000/hook|http://otro/hook,pagos=http://..."`
//! - `SECRETO_SALIDA`, `DATABASE_URL` (sqlite://webhooks.db), `PUERTO` (3000)

use std::collections::HashMap;
use webhooks::*;

fn pares(v: &str) -> HashMap<String, String> {
    v.split(',').filter_map(|p| p.split_once('=')).map(|(a, b)| (a.trim().into(), b.trim().into())).collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.into());
    let cfg = Config {
        secretos: pares(&env("FUENTES", "demo=secreto-demo")),
        destinos: pares(&env("DESTINOS", "")).into_iter().map(|(k, v)| (k, v.split('|').map(String::from).collect())).collect(),
        secreto_salida: env("SECRETO_SALIDA", "salida-demo"),
        tolerancia_seg: 300,
        max_intentos: 6,
        backoff_base_seg: 5,
        max_cuerpo: 1024 * 1024,
    };
    let db = crear_pool(&env("DATABASE_URL", "sqlite://webhooks.db")).await?;
    let reloj: Reloj = std::sync::Arc::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0));
    let estado = Estado::new(db, cfg, reloj);
    // Worker en segundo plano.
    let w = estado.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = procesar_pendientes(&w).await {
                eprintln!("worker: {e}");
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", env("PUERTO", "3000"))).await?;
    println!("Webhooks en http://{}", l.local_addr()?);
    axum::serve(l, app(estado)).await?;
    Ok(())
}
