# 19 · Backend de blog (posts, comentarios, etiquetas)

## Qué demuestra
- Modelo relacional real en SQLite: 1-N (post → comentarios, `ON DELETE CASCADE`) y N-M (posts ↔ etiquetas con tabla puente).
- **Transacciones** SQLx (`begin/commit`, rollback automático al hacer drop) para crear post + etiquetas de forma atómica.
- Arquitectura en capas: `repo.rs` (todo el SQL) y `lib.rs` (HTTP).
- Slugs automáticos (sin tildes, únicos con sufijo `-2`), etiquetas normalizadas, borradores ocultos, comentarios solo en publicados, nube de etiquetas con `GROUP BY`.

## Endpoints
Ver tabla en `src/lib.rs` (`/posts`, `/posts/{slug}`, `/posts/{slug}/comentarios`, `/etiquetas`).

## Uso
```bash
cargo run
curl -XPOST localhost:3000/posts -H 'content-type: application/json' \
  -d '{"titulo":"Hola","cuerpo":"...","publicado":true,"etiquetas":["rust"]}'
curl localhost:3000/posts/hola
```

## Tests
`cargo test`: slugs, repositorio (slugs únicos, normalización de etiquetas, nube), flujo HTTP completo, cascada verificada en la base y validaciones.

## Limitaciones
- Sin autenticación de autores (combinable con el proyecto 18) ni moderación de comentarios.
- Sin paginación ni búsqueda de texto completo (SQLite FTS5 sería el siguiente paso).
- Cuerpo en texto plano (sin render de Markdown).
