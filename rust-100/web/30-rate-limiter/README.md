# 30 · Rate limiter como middleware de Axum (Tower)

Middleware propio que implementa los traits `tower::Layer` y `tower::Service` para limitar peticiones con **token bucket** por cliente.

## Qué demuestra
- **Token bucket** (`src/balde.rs`): ráfaga + ritmo de recarga, cálculo perezoso (sin timers), reloj que no retrocede, `Retry-After` exacto.
- **Middleware Tower genérico** (`src/lib.rs`): `LimiteLayer` → `Limite<S>`; sirve para `Router::layer` (global) y `route_layer` (por ruta). Ejemplo: límite general por IP y uno **más estricto para `/login`** (anti fuerza bruta).
- **Claves configurables**: `por_ip()` (vía `ConnectInfo`, sin confiar en `X-Forwarded-For`) o `por_header("x-api-key")`; sin clave → balde compartido `anonimo`.
- Respuestas: `429` + `Retry-After`; en las permitidas `X-RateLimit-Limit` / `X-RateLimit-Remaining`.
- **Memoria acotada**: `max_claves`; los baldes **llenos** se purgan (equivalen a uno nuevo); si la tabla está llena de clientes activos → `503`.
- El `Mutex` nunca se sostiene mientras corre el handler; patrón `mem::replace` de Tower para el servicio "listo".

## Uso
```bash
cargo run    # 10 de ráfaga, 5/s por IP; /login: 3 de ráfaga, 1 cada 10 s
for i in $(seq 15); do curl -s -o /dev/null -w '%{http_code}\n' localhost:3000/; done
```
En tu app:
```rust
let capa = LimiteLayer::new(Config { rafaga: 20.0, por_segundo: 10.0, max_claves: 100_000 }, por_ip());
let app = Router::new().route("/", get(h)).layer(capa);
axum::serve(l, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
```

## Tests
`cargo test` → 2 unitarios del algoritmo + 4 de integración (reloj falso): cabeceras y 429 por IP, límite estricto en `/login` y purga, clave por header y anónimos, y un servidor TCP real con `ConnectInfo`.

## Limitaciones
- Estado en memoria de un proceso: con varias réplicas hace falta un almacén compartido (Redis + script Lua).
- Un único `Mutex` global; para muchísimo tráfico, fragmentar (sharding) o usar `DashMap`.
- La IP detrás de un proxy requiere configurar una clave basada en un header de confianza.
