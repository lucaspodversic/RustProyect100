//! Arranque: `ADMIN_USUARIO=admin ADMIN_CLAVE=secreto123 cargo run`
//! Vars: DATABASE_URL (sqlite://portal.db), PUERTO (3000), COOKIE_SEGURA=1 detrás de HTTPS.

use portal_admin::{app, crear_pool, crear_usuario, Estado};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://portal.db".into());
    let mut e = Estado::nuevo(crear_pool(&url).await?);
    e.cookie_segura = std::env::var("COOKIE_SEGURA").is_ok_and(|v| v == "1");
    // Admin inicial (idempotente: si ya existe, sólo avisa).
    if let (Ok(u), Ok(c)) = (std::env::var("ADMIN_USUARIO"), std::env::var("ADMIN_CLAVE")) {
        match crear_usuario(&e, &u, &c, "admin").await {
            Ok(_) => println!("Admin '{u}' creado"),
            Err(m) => println!("Admin '{u}': {m}"),
        }
    }
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("Portal en http://{}/login", l.local_addr()?);
    axum::serve(l, app(e)).await?;
    Ok(())
}
