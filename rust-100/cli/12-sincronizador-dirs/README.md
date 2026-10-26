# 12 · Sincronizador de directorios (rsync simplificado)

`rsincro ORIGEN DESTINO` deja el destino igual al origen (unidireccional).

## Qué demuestra
- Diseño **plan → ejecución**: `planificar` es puro respecto del destino (no escribe) y permite `--simular`.
- Comparación rápida (tamaño + mtime con tolerancia de 2 s) o exacta (`--checksum`, BLAKE3).
- Copia **atómica** (archivo temporal + `rename`) y preservación de mtime (`File::set_modified`) para que la sincronización sea idempotente.
- Borrado de sobrantes opcional (`--borrar`), en orden hijos → padres; manejo de cambio de tipo archivo↔directorio.
- Exclusiones simples (`-x .git -x '*.log'`).

## Uso
```bash
cargo run -- ./fotos /mnt/backup/fotos -n          # ver plan
cargo run -- ./fotos /mnt/backup/fotos --borrar    # aplicar
cargo run -- src dst -c -x target -x '*.tmp'
```

## Tests
`cargo test`: copia inicial + idempotencia, cambios por tamaño, checksum vs. modo rápido, borrado opcional y ordenado, exclusiones, cambio de tipo. Usan directorios temporales (`tempfile`).

## Limitaciones
- Sin transferencia por red ni algoritmo delta de rsync (copia archivos completos).
- No preserva permisos/dueño/xattrs; los symlinks se siguen (se copia el contenido).
- Exclusiones sin globs completos.
