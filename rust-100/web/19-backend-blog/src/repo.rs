//! Capa de datos (repositorio): todas las consultas SQL viven acá.
//! Los handlers HTTP no ven SQL; así se pueden testear ambas capas por separado.
//!
//! Esquema:
//! ```text
//! posts(id, slug UNIQUE, titulo, cuerpo, publicado, creado)
//! comentarios(id, post_id → posts ON DELETE CASCADE, autor, texto, creado)
//! etiquetas(id, nombre UNIQUE)
//! post_etiquetas(post_id, etiqueta_id)   ← relación muchos-a-muchos
//! ```

use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, sqlx::FromRow, PartialEq)]
pub struct Post {
    pub id: i64,
    pub slug: String,
    pub titulo: String,
    pub cuerpo: String,
    pub publicado: bool,
    pub creado: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, PartialEq)]
pub struct Comentario {
    pub id: i64,
    pub autor: String,
    pub texto: String,
    pub creado: String,
}

/// Post con sus relaciones, para la vista de detalle.
#[derive(Debug, Serialize)]
pub struct PostCompleto {
    #[serde(flatten)]
    pub post: Post,
    pub etiquetas: Vec<String>,
    pub comentarios: Vec<Comentario>,
}

#[derive(Debug, Deserialize)]
pub struct DatosPost {
    pub titulo: String,
    pub cuerpo: String,
    #[serde(default)]
    pub publicado: bool,
    #[serde(default)]
    pub etiquetas: Vec<String>,
}

/// Convierte un título en slug URL-amigable: minúsculas, sin tildes, guiones.
/// "¡Hola, Año Nuevo!" → "hola-ano-nuevo".
pub fn slugificar(t: &str) -> String {
    let mut s = String::new();
    for c in t.to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-'); // cualquier separador colapsa en un solo guion
        }
    }
    s.trim_end_matches('-').to_string()
}

/// Normaliza etiquetas: minúsculas, sin espacios extremos, sin vacías ni repetidas.
pub fn normalizar_etiquetas(v: &[String]) -> Vec<String> {
    let mut e: Vec<String> = v.iter().map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect();
    e.sort();
    e.dedup();
    e
}

pub async fn crear_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    // foreign_keys(true): SQLite NO aplica claves foráneas (ni CASCADE) si no se activa.
    let o = SqliteConnectOptions::from_str(url)?.create_if_missing(true).foreign_keys(true);
    let pool = SqlitePoolOptions::new().max_connections(if url.contains(":memory:") { 1 } else { 5 }).connect_with(o).await?;
    for ddl in [
        "CREATE TABLE IF NOT EXISTS posts (id INTEGER PRIMARY KEY AUTOINCREMENT, slug TEXT NOT NULL UNIQUE,
            titulo TEXT NOT NULL, cuerpo TEXT NOT NULL, publicado BOOLEAN NOT NULL DEFAULT 0,
            creado TEXT NOT NULL DEFAULT (datetime('now')))",
        "CREATE TABLE IF NOT EXISTS comentarios (id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
            autor TEXT NOT NULL, texto TEXT NOT NULL, creado TEXT NOT NULL DEFAULT (datetime('now')))",
        "CREATE TABLE IF NOT EXISTS etiquetas (id INTEGER PRIMARY KEY AUTOINCREMENT, nombre TEXT NOT NULL UNIQUE)",
        "CREATE TABLE IF NOT EXISTS post_etiquetas (
            post_id INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
            etiqueta_id INTEGER NOT NULL REFERENCES etiquetas(id) ON DELETE CASCADE,
            PRIMARY KEY (post_id, etiqueta_id))",
        "CREATE INDEX IF NOT EXISTS idx_coment_post ON comentarios(post_id)",
    ] {
        sqlx::query(ddl).execute(&pool).await?;
    }
    Ok(pool)
}

/// Reemplaza las etiquetas de un post (dentro de una transacción ya abierta).
async fn fijar_etiquetas(tx: &mut Transaction<'_, Sqlite>, post_id: i64, etiquetas: &[String]) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM post_etiquetas WHERE post_id = ?").bind(post_id).execute(&mut **tx).await?;
    for e in normalizar_etiquetas(etiquetas) {
        // "Upsert" de la etiqueta y obtención de su id.
        sqlx::query("INSERT OR IGNORE INTO etiquetas (nombre) VALUES (?)").bind(&e).execute(&mut **tx).await?;
        sqlx::query("INSERT INTO post_etiquetas (post_id, etiqueta_id) SELECT ?, id FROM etiquetas WHERE nombre = ?")
            .bind(post_id)
            .bind(&e)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// Crea un post; si el slug ya existe agrega sufijo `-2`, `-3`...
pub async fn crear_post(db: &SqlitePool, d: &DatosPost) -> Result<Post, sqlx::Error> {
    let mut tx = db.begin().await?;
    let base = slugificar(&d.titulo);
    let base = if base.is_empty() { "post".to_string() } else { base };
    let mut slug = base.clone();
    let mut n = 2;
    while sqlx::query("SELECT 1 FROM posts WHERE slug = ?").bind(&slug).fetch_optional(&mut *tx).await?.is_some() {
        slug = format!("{base}-{n}");
        n += 1;
    }
    let p = sqlx::query_as::<_, Post>(
        "INSERT INTO posts (slug, titulo, cuerpo, publicado) VALUES (?, ?, ?, ?) RETURNING *",
    )
    .bind(&slug)
    .bind(d.titulo.trim())
    .bind(&d.cuerpo)
    .bind(d.publicado)
    .fetch_one(&mut *tx)
    .await?;
    fijar_etiquetas(&mut tx, p.id, &d.etiquetas).await?;
    tx.commit().await?; // si algo falla antes, el Drop de tx hace ROLLBACK
    Ok(p)
}

/// Actualiza título/cuerpo/publicado/etiquetas (el slug se mantiene: URLs estables).
pub async fn actualizar_post(db: &SqlitePool, slug: &str, d: &DatosPost) -> Result<Option<Post>, sqlx::Error> {
    let mut tx = db.begin().await?;
    let p = sqlx::query_as::<_, Post>("UPDATE posts SET titulo = ?, cuerpo = ?, publicado = ? WHERE slug = ? RETURNING *")
        .bind(d.titulo.trim())
        .bind(&d.cuerpo)
        .bind(d.publicado)
        .bind(slug)
        .fetch_optional(&mut *tx)
        .await?;
    if let Some(p) = &p {
        fijar_etiquetas(&mut tx, p.id, &d.etiquetas).await?;
    }
    tx.commit().await?;
    Ok(p)
}

pub async fn borrar_post(db: &SqlitePool, slug: &str) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM posts WHERE slug = ?").bind(slug).execute(db).await?.rows_affected() > 0)
}

/// Lista posts; `solo_publicados` oculta borradores; filtro opcional por etiqueta.
pub async fn listar(db: &SqlitePool, solo_publicados: bool, etiqueta: Option<&str>) -> Result<Vec<Post>, sqlx::Error> {
    sqlx::query_as::<_, Post>(
        "SELECT p.* FROM posts p
         WHERE (? = 0 OR p.publicado = 1)
           AND (? IS NULL OR EXISTS (SELECT 1 FROM post_etiquetas pe JOIN etiquetas e ON e.id = pe.etiqueta_id
                                      WHERE pe.post_id = p.id AND e.nombre = ?))
         ORDER BY p.creado DESC, p.id DESC",
    )
    .bind(solo_publicados)
    .bind(etiqueta)
    .bind(etiqueta.map(str::to_lowercase))
    .fetch_all(db)
    .await
}

pub async fn obtener(db: &SqlitePool, slug: &str) -> Result<Option<PostCompleto>, sqlx::Error> {
    let Some(post) = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE slug = ?").bind(slug).fetch_optional(db).await? else {
        return Ok(None);
    };
    let etiquetas = sqlx::query_scalar::<_, String>(
        "SELECT e.nombre FROM etiquetas e JOIN post_etiquetas pe ON pe.etiqueta_id = e.id WHERE pe.post_id = ? ORDER BY e.nombre",
    )
    .bind(post.id)
    .fetch_all(db)
    .await?;
    let comentarios = sqlx::query_as::<_, Comentario>("SELECT id, autor, texto, creado FROM comentarios WHERE post_id = ? ORDER BY id")
        .bind(post.id)
        .fetch_all(db)
        .await?;
    Ok(Some(PostCompleto { post, etiquetas, comentarios }))
}

/// Agrega comentario; `None` si el post no existe o no está publicado.
pub async fn comentar(db: &SqlitePool, slug: &str, autor: &str, texto: &str) -> Result<Option<Comentario>, sqlx::Error> {
    sqlx::query_as::<_, Comentario>(
        "INSERT INTO comentarios (post_id, autor, texto)
         SELECT id, ?, ? FROM posts WHERE slug = ? AND publicado = 1
         RETURNING id, autor, texto, creado",
    )
    .bind(autor.trim())
    .bind(texto.trim())
    .bind(slug)
    .fetch_optional(db)
    .await
}

/// Etiquetas con cantidad de posts (nube de etiquetas). Omite las huérfanas.
pub async fn nube_etiquetas(db: &SqlitePool) -> Result<Vec<(String, i64)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT e.nombre, COUNT(pe.post_id) AS n FROM etiquetas e JOIN post_etiquetas pe ON pe.etiqueta_id = e.id
         GROUP BY e.id ORDER BY n DESC, e.nombre",
    )
    .fetch_all(db)
    .await
}
