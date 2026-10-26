# 22 · Subida y descarga de archivos (límites de tamaño y MIME)

## Qué demuestra
- `Multipart` de Axum leído **en trozos** con corte temprano (413) + `DefaultBodyLimit` como segunda barrera.
- Detección de tipo por **magic bytes** (`src/mime.rs`), no por extensión ni `Content-Type` del cliente; lista blanca → 415.
- Almacenamiento **direccionado por contenido** (nombre en disco = hash BLAKE3): deduplicación y sin path traversal.
- Escritura atómica (temporal + `rename`), metadatos JSON al lado, descarga en *streaming* (`ReaderStream`) con `Content-Disposition: attachment` y `nosniff`.
- Saneado del nombre original (solo último componente, sin comillas/controles).

## Uso
```bash
DIR=./subidas MAX_BYTES=1048576 cargo run
curl -F archivo=@foto.png localhost:3000/archivos
curl -OJ localhost:3000/archivos/<id>
```

## Tests
`cargo test`: firmas MIME, saneado de nombres, flujo completo con deduplicación y headers, límites (por campo y por body), ejecutable disfrazado, campo faltante, IDs inválidos.

## Limitaciones
- Sin autenticación ni cuotas por usuario; borrar un archivo deduplicado lo borra para todos los que lo subieron.
- La detección MIME cubre un puñado de formatos (sin SVG: es XML con posible JS y no está permitido).
