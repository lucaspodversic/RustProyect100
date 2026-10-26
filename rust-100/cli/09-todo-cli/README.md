# 09 · To-Do CLI con persistencia JSON

`todo` gestiona tareas con prioridad, fecha límite y etiquetas, guardadas en un JSON local con escritura atómica.

## Qué demuestra
- Modelo serializable con `serde` (enums en minúsculas, campos opcionales con `skip_serializing_if`).
- **Escritura atómica** (archivo `.tmp` + `rename`).
- Subcomandos `clap`, argumento con variable de entorno (`TODO_ARCHIVO`).
- Ordenamiento compuesto (pendientes → prioridad → vencimiento → id) y filtros combinables.
- Validación de fechas sin dependencias (bisiestos incluidos).

## Uso
```bash
cargo run -- add "Pagar luz" -p alta -v 2024-06-10 -e casa
cargo run -- ls --pendientes
cargo run -- ls -e casa --vence-hasta 2024-06-30
cargo run -- done 1 && cargo run -- stats
TODO_ARCHIVO=./mis-tareas.json cargo run -- ls
```

## Tests
`cargo test`: IDs, validaciones, orden y filtros, edición/borrado, roundtrip de persistencia y JSON corrupto.

## Limitaciones
- Sin bloqueo entre procesos: dos `todo` simultáneos pueden pisarse (último en escribir gana).
- Se eligió JSON sobre SQLite por simplicidad y legibilidad.
