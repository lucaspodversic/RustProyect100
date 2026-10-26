# 02 · Parser de logs con filtros y estadísticas

CLI `logparse` que lee un log (archivo o stdin), filtra por **nivel mínimo**, **rango de fechas** y **patrón regex**, y muestra estadísticas: conteo por nivel e **histograma de errores por hora**.

## Qué demuestra
- Parsing de texto con una regex compilada una sola vez (`Parser`).
- Enums con orden (`PartialOrd`) para comparar severidades; `FromStr` para usarlos directamente en `clap`.
- Agregaciones con `BTreeMap` (salida ordenada y determinista).

## Formato soportado
`AAAA-MM-DD[T| ]HH:MM:SS[.fracción][Z] NIVEL mensaje` — niveles TRACE, DEBUG, INFO, WARN/WARNING, ERROR/ERR/FATAL.

## Uso
```bash
cargo run -- app.log --nivel warn --stats
cargo run -- app.log --desde "2024-05-01 10:00:00" --hasta 2024-05-01 -p 'timeout'
cat app.log | cargo run -- - --stats -q
```

## Tests
`cargo test` (parseo, filtros por nivel/fecha/patrón, estadísticas).

## Limitaciones
- Carga el archivo completo en memoria.
- No interpreta zonas horarias: compara las marcas como texto ISO.
- Formatos multi-línea (stack traces) cuentan como líneas no parseables.
