# 26 · API GraphQL con async-graphql

## Qué demuestra
- Esquema **code-first**: tipos Rust → SDL (`cargo run -- --sdl`).
- Queries con resolvers de campo anidados (`Autor.libros`, `Libro.autor`), filtros con `InputObject`, paginación con `total/hayMas`.
- Mutations con validación y **errores con `extensions.codigo`** (`NO_ENCONTRADO`, `INVALIDO`, `NO_DISPONIBLE`).
- **Subscriptions** (`libroCreado`) con `tokio::sync::broadcast` sobre WebSocket (`/ws`).
- Seguridad: `limit_depth(6)` y `limit_complexity(300)` contra consultas recursivas abusivas.
- Integración con Axum 0.8 (`async-graphql-axum`) y GraphiQL en `GET /graphql`.

## Uso
```bash
cargo run   # abrir http://localhost:3000/graphql
```
```graphql
mutation { crearAutor(nombre: "Borges") { id } }
mutation { crearLibro(datos: {titulo: "Ficciones", anio: 1944, autorId: "1"}) { id } }
{ autores { nombre libros { titulo disponible } } }
subscription { libroCreado { titulo autor { nombre } } }
```

## Tests
Ejecución directa del esquema (sin HTTP): consultas anidadas, filtros y paginación, errores con código, préstamo/devolución, variables, límite de profundidad, suscripción con `execute_stream`; y un POST real vía router Axum.

## Limitaciones
- Datos en memoria (se pierden al reiniciar). Sin DataLoader: `Autor.libros` recorre la lista (bien para pocos datos, N+1 con una base real).
- Introspección habilitada (en producción conviene deshabilitarla o protegerla).

## Nota de versiones
El `Cargo.lock` fija `async-graphql-{derive,parser,value}` en **7.0.16**: las 7.2.x requieren rustc ≥ 1.88 (usan *let chains* y `darling 0.23`). Con un compilador más nuevo, `cargo update` las sube sin problemas.
