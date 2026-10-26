//! Punto de entrada del navegador (solo wasm32, con `trunk build --features hydrate`).
//! Lee las notas iniciales que el servidor dejó en `<script id="datos">` y
//! **hidrata** el HTML ya renderizado.

#[cfg(target_arch = "wasm32")]
fn main() {
    let notas: Vec<compartido::Nota> = web_sys_datos().unwrap_or_default();
    yew::Renderer::<frontend::App>::with_props(frontend::PropsApp { notas }).hydrate();
}

#[cfg(target_arch = "wasm32")]
fn web_sys_datos() -> Option<Vec<compartido::Nota>> {
    let doc = gloo_utils_document()?;
    let txt = doc.get_element_by_id("datos")?.text_content()?;
    serde_json::from_str(&txt).ok()
}

#[cfg(target_arch = "wasm32")]
fn gloo_utils_document() -> Option<web_sys::Document> {
    web_sys::window()?.document()
}

/// En nativo este binario no tiene sentido: el frontend se usa como
/// biblioteca desde el backend (SSR).
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("El frontend se compila a WASM con: trunk build --release --features hydrate");
    eprintln!("En nativo, usalo vía el backend (SSR): cargo run -p backend");
}
