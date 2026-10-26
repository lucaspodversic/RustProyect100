//! Servidor: DATABASE_URL (sqlite://enlaces.db), SAL (¡cambiar en producción!), PUERTO (3000).

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use acortador_analiticas::*;
    let db = crear_pool(&std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://enlaces.db".into())).await?;
    let sal = std::env::var("SAL").unwrap_or_else(|_| {
        eprintln!("aviso: SAL no definida, usando una de desarrollo");
        "sal-de-desarrollo".into()
    });
    let puerto = std::env::var("PUERTO").unwrap_or_else(|_| "3000".into());
    let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{puerto}")).await?;
    println!("Acortador en http://{}", l.local_addr()?);
    axum::serve(l, app(Estado { db, reloj: reloj_real(), sal: sal.into() })).await?;
    Ok(())
}
