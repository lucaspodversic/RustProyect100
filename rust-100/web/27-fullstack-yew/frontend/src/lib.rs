//! # Frontend Yew
//!
//! Componentes:
//! - [`App`]: raíz; recibe las notas iniciales (del SSR) y mantiene el
//!   estado con `use_reducer` sobre [`estado::EstadoUi`].
//! - [`ItemNota`]: una fila con checkbox y botón borrar.
//!
//! ## Dos modos de ejecución
//! - **Servidor (nativo, feature `ssr`)**: `renderizar(notas)` produce HTML
//!   que el backend envía en la primera carga → la página se ve sin JS.
//! - **Navegador (wasm32, feature `hydrate`)**: `yew::Renderer::hydrate()`
//!   toma ese HTML y le "engancha" los eventos. Las llamadas HTTP
//!   (`api::*`) solo existen en wasm32; en nativo son no-ops.

pub mod estado;

use compartido::Nota;
use estado::{Accion, EstadoUi, Filtro};
use std::rc::Rc;
use yew::prelude::*;

/// Envoltorio para usar `EstadoUi::reducir` con `use_reducer`.
#[derive(Default, PartialEq)]
pub struct Tienda(pub EstadoUi);

impl Reducible for Tienda {
    type Action = Accion;
    fn reduce(self: Rc<Self>, a: Accion) -> Rc<Self> {
        Rc::new(Tienda(self.0.clone().reducir(a)))
    }
}

#[derive(Properties, PartialEq)]
pub struct PropsApp {
    pub notas: Vec<Nota>,
}

#[function_component(App)]
pub fn app(p: &PropsApp) -> Html {
    let tienda = {
        let iniciales = p.notas.clone();
        use_reducer(move || Tienda(EstadoUi { notas: iniciales, ..Default::default() }))
    };
    let e = &tienda.0;

    let al_escribir = {
        let t = tienda.clone();
        Callback::from(move |ev: InputEvent| t.dispatch(Accion::Escribir(api::valor_input(&ev))))
    };
    let al_enviar = {
        let t = tienda.clone();
        Callback::from(move |ev: SubmitEvent| {
            ev.prevent_default();
            t.dispatch(Accion::Enviar);
            api::crear(t.0.borrador.clone(), t.dispatcher());
        })
    };
    let filtro = |f: Filtro, txt: &'static str| {
        let t = tienda.clone();
        let activo = e.filtro == f;
        html! { <button class={classes!(activo.then_some("activo"))} onclick={move |_| t.dispatch(Accion::Filtrar(f))}>{ txt }</button> }
    };

    html! {
        <main>
            <h1>{ "Notas" }</h1>
            <form onsubmit={al_enviar}>
                <input value={e.borrador.clone()} oninput={al_escribir} placeholder="Nueva nota" maxlength="140" />
                <button type="submit">{ "Agregar" }</button>
            </form>
            if let Some(err) = &e.error { <p class="error">{ err }</p> }
            <nav>{ filtro(Filtro::Todas, "Todas") }{ filtro(Filtro::Pendientes, "Pendientes") }{ filtro(Filtro::Hechas, "Hechas") }</nav>
            <ul>
                { for e.visibles().into_iter().map(|n| html! { <ItemNota key={n.id} nota={n.clone()} despachar={tienda.dispatcher()} /> }) }
            </ul>
            <p class="contador">{ format!("{} pendiente(s)", e.pendientes()) }</p>
        </main>
    }
}

#[derive(Properties, PartialEq)]
pub struct PropsItem {
    pub nota: Nota,
    pub despachar: UseReducerDispatcher<Tienda>,
}

#[function_component(ItemNota)]
pub fn item_nota(p: &PropsItem) -> Html {
    let n = p.nota.clone();
    let alternar = {
        let (d, id) = (p.despachar.clone(), n.id);
        Callback::from(move |_| api::alternar(id, d.clone()))
    };
    let borrar = {
        let (d, id) = (p.despachar.clone(), n.id);
        Callback::from(move |_| api::borrar(id, d.clone()))
    };
    html! {
        <li class={classes!(n.hecha.then_some("hecha"))}>
            <input type="checkbox" checked={n.hecha} onclick={alternar} />
            // Yew escapa el texto automáticamente: sin riesgo de XSS.
            <span>{ &n.texto }</span>
            <button onclick={borrar} aria-label="borrar">{ "✕" }</button>
        </li>
    }
}

/// Render del lado del servidor (SSR) a HTML.
#[cfg(feature = "ssr")]
pub async fn renderizar(notas: Vec<Nota>) -> String {
    yew::ServerRenderer::<App>::with_props(move || PropsApp { notas }).render().await
}

/// Efectos de red. En wasm32 hablan con el backend; en nativo no hacen nada
/// (el SSR nunca dispara eventos, y así el crate compila sin target wasm).
pub mod api {
    use super::*;

    #[cfg(target_arch = "wasm32")]
    pub fn valor_input(ev: &InputEvent) -> String {
        use wasm_bindgen::JsCast;
        ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|i| i.value()).unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn valor_input(_ev: &InputEvent) -> String {
        String::new()
    }

    #[cfg(target_arch = "wasm32")]
    mod red {
        use super::*;
        use gloo_net::http::Request;

        /// Ejecuta una petición y despacha el resultado (o el error).
        fn lanzar<F>(d: UseReducerDispatcher<Tienda>, f: F)
        where
            F: std::future::Future<Output = Result<Accion, gloo_net::Error>> + 'static,
        {
            wasm_bindgen_futures::spawn_local(async move {
                d.dispatch(f.await.unwrap_or_else(|e| Accion::ErrorRed(e.to_string())));
            });
        }

        pub fn crear(texto: String, d: UseReducerDispatcher<Tienda>) {
            if compartido::validar_texto(&texto).is_err() {
                return; // el reductor ya mostró el error
            }
            lanzar(d, async move {
                let r = Request::post("/api/notas").json(&compartido::NuevaNota { texto })?.send().await?;
                Ok(Accion::Creada(r.json().await?))
            });
        }
        pub fn alternar(id: u32, d: UseReducerDispatcher<Tienda>) {
            lanzar(d, async move { Ok(Accion::Actualizada(Request::post(&format!("/api/notas/{id}/alternar")).send().await?.json().await?)) });
        }
        pub fn borrar(id: u32, d: UseReducerDispatcher<Tienda>) {
            lanzar(d, async move {
                Request::delete(&format!("/api/notas/{id}")).send().await?;
                Ok(Accion::Borrada(id))
            });
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub use red::*;

    #[cfg(not(target_arch = "wasm32"))]
    pub fn crear(_t: String, _d: UseReducerDispatcher<Tienda>) {}
    #[cfg(not(target_arch = "wasm32"))]
    pub fn alternar(_id: u32, _d: UseReducerDispatcher<Tienda>) {}
    #[cfg(not(target_arch = "wasm32"))]
    pub fn borrar(_id: u32, _d: UseReducerDispatcher<Tienda>) {}
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ssr_renderiza_notas_y_escapa_html() {
        let html = renderizar(vec![
            Nota { id: 1, texto: "pan".into(), hecha: false },
            Nota { id: 2, texto: "<script>alert(1)</script>".into(), hecha: true },
        ])
        .await;
        assert!(html.contains("<h1>Notas</h1>"));
        assert!(html.contains("pan"));
        assert!(html.contains("1 pendiente(s)"));
        assert!(html.contains("class=\"hecha\""));
        assert!(!html.contains("<script>alert"), "debe escapar: {html}");
        assert!(html.contains("&lt;script&gt;"));
    }
}
