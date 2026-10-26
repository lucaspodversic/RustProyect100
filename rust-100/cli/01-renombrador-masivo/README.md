# 01 · Renombrador masivo con regex

Herramienta CLI que renombra archivos en lote usando expresiones regulares, **con previsualización por defecto** (dry-run): nunca toca el disco salvo que pases `--aplicar`.

## Qué demuestra
- Parsing de argumentos con `clap` (derive).
- Separación lógica pura (`lib.rs`) / interfaz (`main.rs`) para testear sin terminal.
- Detección de conflictos antes de escribir: destinos duplicados, pisar un archivo existente, nombres inválidos (`/`, `..`).
- Renombrado en **dos fases** (origen → temporal → destino) para soportar intercambios `a↔b`.

## Arquitectura
| Archivo | Rol |
|---|---|
| `src/lib.rs` | `planificar`, `aplicar`, `listar_archivos`, `formatear_plan` + tests |
| `src/main.rs` | CLI `renombrador` |

## Uso
```bash
cargo run -- ./fotos '^IMG_(\d+)\.jpg$' 'foto-$1.jpg'            # previsualiza
cargo run -- ./fotos '^IMG_(\d+)\.jpg$' 'foto-$1.jpg' --aplicar  # aplica
cargo run -- ./docs ' ' '_' --todas                               # todas las coincidencias
```

## Tests
```bash
cargo test
```

## Limitaciones
- Solo renombra archivos del directorio indicado (no recursivo).
- Nombres no-UTF8 se muestran con reemplazo (`to_string_lossy`) y podrían no renombrarse bien.
- Si falla a mitad de la fase 2 (p. ej. permisos) pueden quedar archivos `.renombrador-tmp-*`.
