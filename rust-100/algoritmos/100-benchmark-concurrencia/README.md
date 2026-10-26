# 100 · Benchmark de concurrencia: `std::thread` vs tokio vs rayon

Este proyecto resuelve **la misma tarea** con 8 estrategias de CPU y 5 de I/O. Los tests comprueban que todas dan el mismo resultado, y el binario mide cuánto tarda cada una.

## Estrategias

**CPU‑bound** (`src/cpu.rs`): contar los primos menores que N por división de prueba, repartidos en T tareas. La carga está desequilibrada a propósito, porque los números altos cuestan más.

| Estrategia | Idea |
|---|---|
| `Secuencial` | Referencia. |
| `HilosBloques` | `thread::scope` con un bloque fijo de tareas por hilo (reparto estático). |
| `HiloPorTarea` | Un hilo del SO por tarea. |
| `HilosColaAtomica` | Un hilo por núcleo que toma tareas con `AtomicUsize::fetch_add` (reparto dinámico). |
| `HilosCanal` | Igual que la anterior, pero envía los resultados por `mpsc` (productor/consumidor). |
| `Rayon` | `par_iter().with_max_len(1)` (*work stealing*). |
| `TokioSpawnBlocking` | `spawn_blocking`: la forma correcta de calcular dentro de tokio. |
| `TokioSpawn` | Calcula dentro de `async`. Funciona, pero **bloquea los workers** (antipatrón si a la vez hubiera I/O). |

**I/O‑bound** (`src/io.rs`): n "peticiones" que solo esperan `ms`.

| Estrategia | Idea |
|---|---|
| `Tokio` | `tokio::time::sleep` + `JoinSet`. |
| `TokioLimitado(k)` | Igual, con un `Semaphore` de k permisos para no saturar un servidor. |
| `HiloPorPeticion` | Un hilo del SO por petición (pila de 64 KiB). |
| `Rayon` | `thread::sleep` en el pool: **herramienta equivocada**. |
| `Secuencial` | Una petición detrás de otra. |

## Resultados (8 núcleos, `cargo run --release`)

```
── CPU: primos < 3 000 000 en 64 tareas ──
  Secuencial               171.6 ms  speedup  1.00x
  HilosBloques          30–45    ms  speedup  ~4-5x   ← reparto estático: un hilo se queda con los números grandes
  HiloPorTarea              25.4 ms  speedup  6.74x
  HilosColaAtomica          23.6 ms  speedup  7.26x
  HilosCanal                23.4 ms  speedup  7.34x
  Rayon                     24.0 ms  speedup  7.15x
  TokioSpawnBlocking        25.4 ms  speedup  6.76x
  TokioSpawn                23.9 ms  speedup  7.17x

── I/O: 2000 peticiones de 20 ms ──
  Tokio                     23.9 ms                ← casi el ideal (20 ms) con 8 hilos
  TokioLimitado(100)       425.0 ms                ← 2000/100 = 20 rondas × 20 ms, como se esperaba
  HiloPorPeticion           71.0 ms                ← funciona, pero crea 2000 hilos del SO
  Rayon                   5040.6 ms  (extrapolado) ← 2000/8 rondas × 20 ms
  Secuencial             40282.8 ms  (extrapolado)
```

## Conclusiones

1. **CPU**: lo que más importa es el **reparto dinámico**, no la librería que se use. La cola atómica escrita a mano, rayon y tokio con `spawn_blocking` quedan a la par. El reparto estático pierde por desequilibrio de carga.
2. Rayon necesitó `with_max_len(1)`. Con solo 64 tareas desiguales, su división adaptativa agrupaba varias y dejaba un rezagado (35 ms → 24 ms). El comentario en el código lo explica.
3. **I/O**: tokio solapa miles de esperas con pocos hilos. Un hilo del SO por petición escala hasta cierto punto (memoria y llamadas al sistema). Rayon con `sleep` bloquea el pool.
4. Regla práctica: **rayon para calcular, tokio para esperar** y `spawn_blocking` para el puente entre ambos.

## Uso

```bash
cargo run --release
cargo run --release -- --limite 10000000 --tareas 256 --peticiones 10000 --ms 20
cargo test     # 2 unitarios + 4 de integración
```

Los tests comprueban, entre otras cosas, que:

- 8 estrategias × {1, 3, 8} hilos devuelven 17 984 primos (π(200 000));
- tokio completa 5000 esperas de 50 ms en 2 hilos en menos de 3 s;
- rayon con `sleep` y el semáforo **sí** limitan la concurrencia (cotas inferiores de tiempo).

## Limitaciones

- Las cifras dependen de la máquina; las de arriba se obtuvieron en una VM de 8 vCPU.
- Cada medición incluye la creación del pool o del runtime (~1 ms), igual para todas las estrategias.
- El I/O es simulado con `sleep`; no hay red real.
- Las estrategias lentas se extrapolan a partir de una muestra.
