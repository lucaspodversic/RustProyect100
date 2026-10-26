# 96 · Macro procedural `#[derive(Describir)]`

Este proyecto es un workspace con dos crates:

```
96-macro-derive/
├── Cargo.toml          # crate `describir`: trait + reexport + [workspace]
├── src/lib.rs          # trait Describir, doctests (incluye compile_fail)
├── derive/             # crate `describir-derive` (proc-macro = true)
│   └── src/lib.rs      # parseo con syn 2 y generación con quote
├── tests/derive.rs     # 7 tests de integración
└── examples/demo.rs
```

## Qué genera

```rust
#[derive(Describir)]
#[describir(nombre = "Cfg")]          // nombre visible del tipo
struct Configuracion {
    host: &'static str,
    #[describir(renombrar = "port")]  // otra etiqueta
    puerto: u16,
    #[describir(omitir)]              // se oculta (aparece "..")
    token: String,
}
// describir() → Cfg { host: "localhost", port: 8080, .. }
// campos()    → ["host", "port"]
```

Soporta los siguientes tipos:

- Structs con campos con nombre.
- Structs tupla.
- Structs unidad.
- Enums con cualquier tipo de variante (y con `renombrar` en las variantes).
- **Genéricos**: la macro añade automáticamente `T: Debug` a cada parámetro de tipo.

## Errores de compilación con buen *span*

```
error: atributo de campo desconocido; se admite: omitir, renombrar = "..."
 --> examples/error.rs:3:24
  |
3 | struct X { #[describir(ocultar)] a: u8 }
  |                        ^^^^^^^
```

También se rechazan las `union` y los enums sin variantes. Estos errores se prueban con doctests `compile_fail` en `src/lib.rs`.

## Uso

```bash
cargo test --workspace      # 1 unitario + 7 de integración + 4 doctests
cargo run --example demo
cargo expand --example demo # (opcional, requiere cargo-expand) ver el código generado
```

## Conceptos clave (comentados en el código)

- **Por qué hacen falta dos crates**: un `proc-macro` se compila para el *host* y se carga en rustc.
- `#[proc_macro_derive(Describir, attributes(describir))]` registra el atributo auxiliar.
- `parse_nested_meta` de syn 2 sirve para leer `clave` y `clave = "valor"`.
- `syn::Error::new_spanned(...).to_compile_error()` produce errores amigables en lugar de *panics*.
- `split_for_impl()` permite propagar los genéricos y las cláusulas `where`.
- Las rutas son absolutas (`::describir::Describir`, `::std::format!`), con `extern crate self as describir;` para que funcionen también dentro del propio crate (higiene).
- Los campos omitidos se enlazan con `_` en el patrón, así que nunca se tocan.

## Limitaciones

- Los campos visibles deben implementar `Debug`.
- No se puede pasar una función de formato propia por campo (sería una extensión natural: `#[describir(con = "ruta")]`).
- No existen tests con `trybuild`; se usan doctests `compile_fail`, que no comprueban el texto exacto del error.
