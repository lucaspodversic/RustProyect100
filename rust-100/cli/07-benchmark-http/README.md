# 07 · Benchmark HTTP concurrente (tipo hey/wrk)

`hbench` dispara N requests GET con C workers concurrentes y reporta req/s, latencia promedio, p50/p90/p99, máximo, códigos de estado y errores.

## Qué demuestra
- Async con `tokio`: una tarea por worker, cada una con **conexión keep-alive** propia.
- Cliente **HTTP/1.1 escrito a mano** sobre `TcpStream` (línea de estado, cabeceras, `Content-Length` y `Transfer-Encoding: chunked`).
- Reparto de trabajo con un contador atómico (`fetch_sub`) sin locks.
- Percentiles por *nearest-rank*.

## Uso
```bash
cargo run --release -- http://127.0.0.1:8080/ -n 5000 -c 64
```

## Tests
`cargo test` levanta un servidor HTTP mínimo en `127.0.0.1:0` (puerto efímero) que alterna respuestas con Content-Length y chunked; también verifica que las conexiones rechazadas se cuenten como errores. No necesita red externa.

## Limitaciones
- Solo `http://` (sin TLS) y solo GET.
- No hay modo por duración (`-z 10s`) ni rate limiting del cliente.
- La latencia incluye la apertura de la conexión en la primera request de cada worker.
