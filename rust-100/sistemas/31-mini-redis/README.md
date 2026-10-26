# 31 · Mini Redis

Almacén clave-valor compatible con un subconjunto de **RESP2** (el protocolo de Redis), con persistencia por snapshot y cliente CLI. Solo biblioteca estándar.

## Qué demuestra
- Parser **incremental** de RESP (soporta comandos partidos en varios `read()` y *pipelining*).
- Servidor TCP multihilo con estado compartido (`Arc<Mutex<Store>>`).
- Expiración de claves (TTL) perezosa, `INCR` con chequeo de overflow, `KEYS` con glob.
- Persistencia atómica (archivo temporal + `rename`) reutilizando RESP como formato.

## Arquitectura
| Módulo | Rol |
|---|---|
| `resp` | Tipos `Value`, `encode()`, `parse()` incremental |
| `store` | `HashMap` + TTL, glob matching |
| `persist` | `save`/`load` de snapshot |
| `server` | `execute()` de comandos + loop por conexión |
| `client` | Cliente bloqueante + formato estilo `redis-cli` |

Comandos: `PING ECHO SET [EX|PX] GET DEL EXISTS INCR DECR INCRBY EXPIRE TTL KEYS DBSIZE FLUSHALL SAVE`.

## Cómo correr
```bash
cargo run -- server 127.0.0.1:6380 dump.resp
cargo run -- cli 127.0.0.1:6380 SET saludo hola
cargo run -- cli 127.0.0.1:6380 GET saludo
# también funciona con el redis-cli real:  redis-cli -p 6380 PING
```

## Tests
`cargo test` — roundtrip RESP, parsing incompleto/inline, glob, TTL, snapshot y servidor end-to-end en `127.0.0.1:0`.

## Limitaciones
Solo strings (sin listas/hashes/sets), sin RESP3, sin replicación ni AOF, un hilo por conexión (no event loop), lock global.
