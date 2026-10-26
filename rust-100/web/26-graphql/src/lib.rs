//! # API GraphQL de una biblioteca (async-graphql + Axum)
//!
//! Esquema (SDL generado automáticamente desde los tipos Rust; ver `cargo run -- --sdl`):
//! ```graphql
//! type Consulta {
//!   autores: [Autor!]!
//!   libro(id: ID!): Libro
//!   libros(filtro: FiltroLibros, limite: Int = 20, desplazamiento: Int = 0): PaginaLibros!
//! }
//! type Mutacion { crearAutor(nombre: String!): Autor!  crearLibro(datos: NuevoLibro!): Libro!  prestar(id: ID!): Libro!  devolver(id: ID!): Libro! }
//! type Suscripcion { libroCreado: Libro! }
//! ```
//!
//! ## Ideas clave
//! - **Resolvers de campo**: `Autor.libros` y `Libro.autor` se calculan solo
//!   si el cliente los pide (GraphQL = el cliente elige la forma).
//! - **Errores de dominio** como errores GraphQL con `extensions.codigo`
//!   (`NO_ENCONTRADO`, `INVALIDO`, `NO_DISPONIBLE`).
//! - **Defensa ante consultas abusivas**: `limit_depth` (anidamiento
//!   autor→libros→autor→… infinito) y `limit_complexity`.
//! - **Subscriptions** sobre WebSocket (`/ws`) usando `tokio::sync::broadcast`.
//! - Almacenamiento en memoria tras `RwLock` (muchos lectores, un escritor).

use async_graphql::{Context, ErrorExtensions, InputObject, Object, Result, Schema, SimpleObject, Subscription, ID};
use futures_util::Stream;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

// ---------- Modelo ----------

#[derive(Clone, Debug)]
pub struct Autor {
    pub id: u32,
    pub nombre: String,
}

#[derive(Clone, Debug)]
pub struct Libro {
    pub id: u32,
    pub titulo: String,
    pub anio: i32,
    pub autor_id: u32,
    pub disponible: bool,
}

#[derive(Default)]
pub struct Almacen {
    autores: Vec<Autor>,
    libros: Vec<Libro>,
}

/// Datos compartidos que se inyectan al esquema (`ctx.data`).
#[derive(Clone)]
pub struct Datos {
    pub almacen: Arc<RwLock<Almacen>>,
    pub eventos: broadcast::Sender<Libro>,
}

impl Default for Datos {
    fn default() -> Self {
        Datos { almacen: Arc::default(), eventos: broadcast::channel(64).0 }
    }
}

/// Construye un error GraphQL con un código legible por máquinas.
fn err(codigo: &'static str, msg: impl Into<String>) -> async_graphql::Error {
    async_graphql::Error::new(msg).extend_with(|_, e| e.set("codigo", codigo))
}

fn parse_id(id: &ID) -> Result<u32> {
    id.parse::<u32>().map_err(|_| err("INVALIDO", "id inválido"))
}

// ---------- Tipos de salida con resolvers ----------

#[Object]
impl Autor {
    async fn id(&self) -> ID {
        ID(self.id.to_string())
    }
    async fn nombre(&self) -> &str {
        &self.nombre
    }
    /// Libros del autor (se resuelve solo si se pide).
    async fn libros(&self, ctx: &Context<'_>) -> Vec<Libro> {
        let a = ctx.data_unchecked::<Datos>().almacen.read().await;
        a.libros.iter().filter(|l| l.autor_id == self.id).cloned().collect()
    }
}

#[Object]
impl Libro {
    async fn id(&self) -> ID {
        ID(self.id.to_string())
    }
    async fn titulo(&self) -> &str {
        &self.titulo
    }
    async fn anio(&self) -> i32 {
        self.anio
    }
    async fn disponible(&self) -> bool {
        self.disponible
    }
    async fn autor(&self, ctx: &Context<'_>) -> Option<Autor> {
        let a = ctx.data_unchecked::<Datos>().almacen.read().await;
        a.autores.iter().find(|x| x.id == self.autor_id).cloned()
    }
}

#[derive(SimpleObject)]
pub struct PaginaLibros {
    pub total: usize,
    pub items: Vec<Libro>,
    pub hay_mas: bool,
}

#[derive(InputObject, Default)]
pub struct FiltroLibros {
    /// Búsqueda por texto en el título (sin distinguir mayúsculas).
    pub texto: Option<String>,
    pub anio_desde: Option<i32>,
    pub solo_disponibles: Option<bool>,
}

#[derive(InputObject)]
pub struct NuevoLibro {
    pub titulo: String,
    pub anio: i32,
    pub autor_id: ID,
}

// ---------- Raíces ----------

pub struct Consulta;

#[Object]
impl Consulta {
    async fn autores(&self, ctx: &Context<'_>) -> Vec<Autor> {
        ctx.data_unchecked::<Datos>().almacen.read().await.autores.clone()
    }

    async fn libro(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Libro>> {
        let id = parse_id(&id)?;
        Ok(ctx.data_unchecked::<Datos>().almacen.read().await.libros.iter().find(|l| l.id == id).cloned())
    }

    /// Paginación por desplazamiento; `limite` acotado a 1..=100.
    async fn libros(&self, ctx: &Context<'_>, filtro: Option<FiltroLibros>, #[graphql(default = 20)] limite: i32, #[graphql(default = 0)] desplazamiento: i32) -> Result<PaginaLibros> {
        if !(1..=100).contains(&limite) || desplazamiento < 0 {
            return Err(err("INVALIDO", "limite debe estar entre 1 y 100 y desplazamiento ≥ 0"));
        }
        let f = filtro.unwrap_or_default();
        let texto = f.texto.map(|t| t.to_lowercase());
        let a = ctx.data_unchecked::<Datos>().almacen.read().await;
        let filtrados: Vec<&Libro> = a
            .libros
            .iter()
            .filter(|l| texto.as_ref().is_none_or(|t| l.titulo.to_lowercase().contains(t)))
            .filter(|l| f.anio_desde.is_none_or(|d| l.anio >= d))
            .filter(|l| !f.solo_disponibles.unwrap_or(false) || l.disponible)
            .collect();
        let (lim, desp) = (limite as usize, desplazamiento as usize);
        Ok(PaginaLibros {
            total: filtrados.len(),
            hay_mas: desp + lim < filtrados.len(),
            items: filtrados.into_iter().skip(desp).take(lim).cloned().collect(),
        })
    }
}

pub struct Mutacion;

#[Object]
impl Mutacion {
    async fn crear_autor(&self, ctx: &Context<'_>, nombre: String) -> Result<Autor> {
        let nombre = nombre.trim().to_string();
        if nombre.is_empty() || nombre.chars().count() > 100 {
            return Err(err("INVALIDO", "nombre requerido (máx. 100)"));
        }
        let mut a = ctx.data_unchecked::<Datos>().almacen.write().await;
        let autor = Autor { id: a.autores.len() as u32 + 1, nombre };
        a.autores.push(autor.clone());
        Ok(autor)
    }

    async fn crear_libro(&self, ctx: &Context<'_>, datos: NuevoLibro) -> Result<Libro> {
        let autor_id = parse_id(&datos.autor_id)?;
        if datos.titulo.trim().is_empty() {
            return Err(err("INVALIDO", "título requerido"));
        }
        if !(-3000..=2100).contains(&datos.anio) {
            return Err(err("INVALIDO", "año fuera de rango"));
        }
        let d = ctx.data_unchecked::<Datos>();
        let libro = {
            let mut a = d.almacen.write().await;
            if !a.autores.iter().any(|x| x.id == autor_id) {
                return Err(err("NO_ENCONTRADO", "el autor no existe"));
            }
            let l = Libro { id: a.libros.len() as u32 + 1, titulo: datos.titulo.trim().into(), anio: datos.anio, autor_id, disponible: true };
            a.libros.push(l.clone());
            l
        }; // soltamos el lock antes de notificar
        let _ = d.eventos.send(libro.clone()); // Err = sin suscriptores
        Ok(libro)
    }

    async fn prestar(&self, ctx: &Context<'_>, id: ID) -> Result<Libro> {
        cambiar_disponible(ctx, &id, false).await
    }

    async fn devolver(&self, ctx: &Context<'_>, id: ID) -> Result<Libro> {
        cambiar_disponible(ctx, &id, true).await
    }
}

async fn cambiar_disponible(ctx: &Context<'_>, id: &ID, nuevo: bool) -> Result<Libro> {
    let id = parse_id(id)?;
    let mut a = ctx.data_unchecked::<Datos>().almacen.write().await;
    let l = a.libros.iter_mut().find(|l| l.id == id).ok_or_else(|| err("NO_ENCONTRADO", "libro inexistente"))?;
    if l.disponible == nuevo {
        return Err(err("NO_DISPONIBLE", if nuevo { "el libro no estaba prestado" } else { "el libro ya está prestado" }));
    }
    l.disponible = nuevo;
    Ok(l.clone())
}

pub struct Suscripcion;

#[Subscription]
impl Suscripcion {
    /// Emite cada libro nuevo. Los mensajes perdidos por lentitud se ignoran.
    async fn libro_creado(&self, ctx: &Context<'_>) -> impl Stream<Item = Libro> {
        use futures_util::StreamExt as _;
        let rx = ctx.data_unchecked::<Datos>().eventos.subscribe();
        tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|r| async move { r.ok() })
    }
}

pub type Esquema = Schema<Consulta, Mutacion, Suscripcion>;

/// Construye el esquema con los límites de seguridad.
pub fn esquema(datos: Datos) -> Esquema {
    Schema::build(Consulta, Mutacion, Suscripcion).data(datos).limit_depth(6).limit_complexity(300).finish()
}

/// Router Axum: POST /graphql, GET /graphql (GraphiQL), WS /ws.
pub fn app(s: Esquema) -> axum::Router {
    use async_graphql::http::GraphiQLSource;
    use async_graphql_axum::{GraphQL, GraphQLSubscription};
    use axum::response::Html;
    use axum::routing::post_service;
    axum::Router::new()
        .route("/graphql", post_service(GraphQL::new(s.clone())).get(|| async { Html(GraphiQLSource::build().endpoint("/graphql").subscription_endpoint("/ws").finish()) }))
        .route_service("/ws", GraphQLSubscription::new(s))
}

