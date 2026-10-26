# 32 · Mini Kafka

Cola de mensajes con topics **particionados**, logs **append-only** en disco y **grupos de consumidores**.

## Qué demuestra
- Particionado por clave con hash FNV-1a determinista (orden garantizado por clave) y round-robin sin clave.
- Log binario `[len][key][len][val]` con recuperación tras reinicio (descarta el último registro truncado).
- Grupos de consumidores con **generaciones**, rebalanceo al entrar/salir miembros y asignación **range**.
- Offsets comprometidos por grupo, cálculo de **lag**, auto-commit en `poll`.
- Protocolo de texto sobre TCP (probable con `nc`).

## Arquitectura
`log::PartitionLog` → `broker::Broker` (topics + groups) → `net` (servidor multihilo + cliente).

## Cómo correr
```bash
cargo run -- 127.0.0.1:9092 ./data
nc 127.0.0.1 9092
CREATE pedidos 3
PRODUCE pedidos cliente-1 compra de 10 unidades
JOIN facturacion worker-1
POLL facturacion pedidos worker-1 10
LAG facturacion pedidos
```

## Tests
`cargo test`: orden por clave, round-robin, asignación range sin solapamiento + rebalanceo, aislamiento entre grupos, persistencia y protocolo TCP en `127.0.0.1:0`.

## Limitaciones
Un solo broker (sin replicación ni líder/seguidores), sin retención/compactación, sin heartbeats (el abandono es explícito con `LEAVE`), offsets comprometidos sólo en memoria.
