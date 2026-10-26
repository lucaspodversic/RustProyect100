# 27 · Full-stack Rust: Yew + Axum

Workspace con tres crates:

| Crate | Rol |
|---|---|
| `compartido/` | Tipos (`Nota`, `NuevaNota`) y **validación compartida** front/back |
| `frontend/` | Componentes **Yew** (`App`, `ItemNota`) + estado como reductor puro (`estado.rs`). Feature `ssr` (nativo) / `hydrate` (WASM) |
| `backend/` | **Axum**: API REST + `GET /` con la app **renderizada en el servidor** por Yew y los datos iniciales embebidos para hidratar |

## Qué demuestra
- Un mismo código de UI que corre en el servidor (SSR → HTML visible sin JS) y en el navegador (hidratación WASM).
- Lógica de UI testeable sin navegador: reductor `EstadoUi::reducir(Accion)`.
- Validación única compartida; escape automático de Yew + JSON seguro dentro de `<script>` (`<` → `\u003c`).
- Código específico de wasm aislado con `#[cfg(target_arch = "wasm32")]` y dependencias por target.

## Uso
```bash
cargo run -p backend                    # http://localhost:3000 (modo solo-SSR)

# Opcional, con el target wasm instalado (no disponible en este entorno):
rustup target add wasm32-unknown-unknown && cargo install wasm-bindgen-cli
cargo build -p frontend --release --target wasm32-unknown-unknown --no-default-features --features hydrate
wasm-bindgen --target web --out-dir frontend/pkg target/wasm32-unknown-unknown/release/frontend.wasm
cargo run -p backend                    # ahora la página es interactiva
```

## Tests (`cargo test` en la raíz del workspace)
- `compartido`: validación.
- `frontend`: reductor (escribir, validar, crear, alternar, filtrar, borrar) y **SSR** (render real de componentes Yew a HTML, escape de `<script>`).
- `backend`: API + página SSR que refleja el estado, rechazo con la validación compartida, JSON embebido a prueba de `</script>`.

## Limitaciones
- **El build WASM no se verificó aquí** (sin `rustup`/target `wasm32`): el código de red/hidratación (`frontend/src/main.rs`, `api::red`) está escrito pero no compilado en este entorno. Todo lo nativo (SSR, lógica, backend) sí compila y se testea.
- Datos en memoria; sin autenticación.
