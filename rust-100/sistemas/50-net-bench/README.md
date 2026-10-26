# 50 · Benchmark de red

Latencia (RTT TCP) con percentiles y throughput (Mbps) contra endpoints locales.

## Cómo correr
```bash
cargo test
# API: measure_latency(addr, n) / measure_throughput(addr, total, chunk)
```

## Limitaciones
Sólo TCP, sin histograma persistente, sin concurrencia multi-conexión.
