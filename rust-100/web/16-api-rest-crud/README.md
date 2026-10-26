# 16 · API REST CRUD con Axum + SQLx (SQLite)

CRUD completo de `productos` con validación, errores JSON consistentes, búsqueda y paginación.

## Qué demuestra
- **Axum 0.8**: `Router`, extractores (`State`, `Path`, `Query`, `Json`), respuestas con código + cabeceras (`201 Created` + `Location`).
- **SQLx 0.8** sin macros de compilación (no requiere base viva para compilar), `query_as` + `FromRow`, `RETURNING`.
- Manejo de errores tipado (`ErrorApi: IntoResponse`): 404, 409 (UNIQUE), 422 (validación), 500 sin filtrar detalles.
- Seguridad: consultas parametrizadas, escape de comodines `LIKE`, límite de página acotado.
- Tests **sin red** con `tower::ServiceExt::oneshot` y SQLite **en memoria**.

## Arquitectura
```
main.rs  → lee DATABASE_URL/PUERTO, crea pool, axum::serve
lib.rs   → crear_pool (esquema), app(pool) → Router, handlers, ErrorApi
tests/   → peticiones HTTP simuladas contra el Router
```

## Uso
```bash
cargo run                                   # sqlite://productos.db en :3000
curl -XPOST localhost:3000/productos -H 'content-type: application/json' \
     -d '{"nombre":"Yerba","precio_centavos":350000,"stock":10}'
curl 'localhost:3000/productos?q=yer&limite=10'
```

## Tests
`cargo test`: ciclo CRUD completo, validaciones, conflicto por nombre duplicado, 404s, búsqueda con comodines escapados, paginación y salud.

## Limitaciones
- Esquema creado con `CREATE TABLE IF NOT EXISTS` (sin herramienta de migraciones versionadas).
- Sin autenticación (ver proyecto 18) ni PATCH parcial.
