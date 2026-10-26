# 34 · Mini Git

Almacenamiento **direccionado por contenido** estilo Git: blobs, trees y commits identificados por SHA-1 del encabezado `tipo tamaño\0contenido`.

## Qué demuestra
- SHA-1 propio (sin crates) y deduplicación automática.
- Object store en `.minigit/objects/ab/cdef…`.
- Trees deterministas (`BTreeMap`), commits con padres, HEAD simbólico y `log` lineal.

## Cómo correr
```bash
cargo test
# API: Repo::init / hash_file / write_tree / commit / log
```

## Limitaciones
Sin zlib, sin index/staging, sin merge, sin packfiles. SHA-1 sólo para content-addressing (no seguridad).
