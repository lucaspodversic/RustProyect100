# 38 · GlobalAlloc personalizado

Bump allocator thread-safe sobre un arena de 1 MiB, con free-list de huecos exactos y estadísticas.

## Cómo usar
```rust
// Por defecto NO se instala como global (rompe libtest).
// Para instalarlo: cargo run --features install
```
Los tests ejercitan `GlobalAlloc::{alloc,dealloc}` directamente.

## Limitaciones
Arena fijo, sin coalescing real, no apto para producción.
