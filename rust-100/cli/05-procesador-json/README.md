# 05 · Procesador de JSON (mini-jq)

CLI `jproc` para **extraer**, **filtrar** y **transformar** JSON desde la terminal, leyendo de archivo o stdin.

## Qué demuestra
- Parser de rutas escrito a mano (`.a.b[0]`, `.items[]`, `.items[-1]`, `."clave con espacios"`).
- Recorrido recursivo de `serde_json::Value` con fan-out (`[]`).
- Mutación segura con `&mut Value` (set con auto-creación de intermedios, delete).
- Subcomandos con `clap` derive.

## Uso
```bash
cargo run -- get '.usuarios[].nombre' datos.json
cargo run -- filter '.edad >= 30' datos.json --ruta .usuarios
cargo run -- filter '.tags ~ "admin"' datos.json --ruta .usuarios
cargo run -- pick nombre,edad datos.json --ruta .usuarios -c
cargo run -- set .config.debug true datos.json
cargo run -- del '.usuarios[0]' datos.json
cat datos.json | cargo run -- keys
```
Operadores: `== != > >= < <= ~` (`~` = contiene en strings/arrays). El valor se lee como JSON; si no lo es, como string.

## Tests
`cargo test`: rutas, fan-out, índices negativos, filtros, pick, set/del.

## Limitaciones
- Un subconjunto pequeño de jq: sin pipes, funciones ni expresiones aritméticas.
- La condición admite un solo operador (sin `and`/`or`).
