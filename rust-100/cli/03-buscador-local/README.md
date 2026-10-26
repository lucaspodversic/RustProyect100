# 03 · Buscador local tipo ripgrep

CLI `buscar` que encuentra archivos por **nombre** (regex/extensión) y por **contenido** (regex), en paralelo con `rayon`.

## Qué demuestra
- Recorrido recursivo con `walkdir` y **poda** de subárboles (`filter_entry`): ignora `.git`, `target`, `node_modules` y ocultos.
- Paralelismo de datos con `rayon` (`par_iter` + `flat_map`).
- Detección de binarios (byte NUL en los primeros 8 KiB), tolerancia a encodings no-UTF8.
- Salida con colores ANSI solo si stdout es TTY; códigos de salida estilo grep (0 = hay coincidencias, 1 = no).

## Uso
```bash
cargo run -- TODO .                 # contenido
cargo run -- TODO src -e rs -i      # solo .rs, sin distinguir mayúsculas
cargo run -- -n '^Cargo' .          # solo por nombre
cargo run -- unwrap . -l            # solo archivos con coincidencias
```

## Tests
`cargo test` crea un árbol temporal (con `target/`, oculto y binario) y verifica filtros y búsqueda.

## Limitaciones
- No lee `.gitignore` (lista fija de ignorados).
- Carga cada archivo completo (límite 50 MiB por archivo en la CLI).
- No sigue symlinks.
