# 15 · Visualizador de dependencias de Cargo como grafo

`cargografo` lee un `Cargo.lock` y muestra el grafo de dependencias: árbol ASCII, Graphviz DOT, "¿por qué tengo X?", duplicados y estadísticas.

## Qué demuestra
- Deserialización de TOML con `serde` + `toml`.
- Resolución de referencias del lock (`"nombre"`, `"nombre versión"`, `"nombre versión (fuente)"`) a índices.
- Algoritmos de grafos a mano: DFS para el árbol (con `(*)` en repetidos), **BFS multi-origen** para el camino más corto, DFS con memo para profundidad.
- Exportación a DOT con colores (workspace en verde, duplicados en naranja).

## Uso
```bash
cargo run -- -l /ruta/a/proyecto arbol -p 2
cargo run -- -l . dot | dot -Tsvg > deps.svg
cargo run -- -l . por-que syn
cargo run -- -l . duplicados
cargo run -- -l . stats
```

## Tests
`cargo test`: lock de ejemplo con dos versiones de `rand` (resolución de versión, árbol, camino, duplicados, stats, DOT) y errores (TOML inválido, referencia inexistente).

## Limitaciones
- `Cargo.lock` no distingue dependencias normales/dev/build ni plataforma ni features (para eso, `cargo metadata`). El grafo es el **superconjunto** resuelto.
- No renderiza imágenes por sí mismo: genera DOT para Graphviz.
