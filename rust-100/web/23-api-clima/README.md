# 23 · API de clima (cliente HTTP mockeable + caché)

## Qué demuestra
- **Inyección de dependencias** con traits: `Proveedor` (servicio externo) y `Reloj` (tiempo). El handler es genérico (`clima::<P, R>`), sin `dyn` ni `async-trait` (usa *async fn in traits*, Rust ≥ 1.75).
- Cliente real con `reqwest` + rustls para [Open-Meteo](https://open-meteo.com) (sin API key), con **timeout** y errores clasificados (transitorio vs formato).
- Caché TTL con clave redondeada (~1 km), capacidad máxima con expulsión del más viejo y **stale-if-error** (degradación elegante).
- Métricas de aciertos/fallos.

## Uso
```bash
cargo run          # requiere internet para datos reales
curl 'localhost:3000/clima?lat=-34.6&lon=-58.4'
curl localhost:3000/metricas
```

## Tests (sin internet)
- `cache.rs`: TTL, vencidas, capacidad.
- `tests/clima.rs`: mock que cuenta llamadas + reloj falso (caché, TTL, stale-if-error, 503, recuperación, métricas); validación de coordenadas; el **cliente real** `OpenMeteo` contra un servidor Axum local que imita la API (respuesta OK, formato inválido, 503, puerto cerrado).

## Limitaciones
- Sin *request coalescing*: dos pedidos simultáneos sin caché generan dos llamadas.
- Caché en memoria por proceso; sin geocodificación (solo lat/lon).
