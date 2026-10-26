# 73 · Indexador de eventos on-chain para una DApp

Indexador de **logs estilo EVM** (como The Graph/Ponder en miniatura) para una DApp con un token ERC-20 y un marketplace.

## Qué hace
- **Ingesta** bloque a bloque desde una `FuenteBloques` (mock; en producción, JSON-RPC).
- **Decodificación** por `topic0 = keccak256(firma)` (`Transfer`, `Approval`, `Listed`, `Sold`).
- **Confirmaciones**: sólo indexa hasta `último - N`.
- **Reorgs**: verifica `padre` contra el hash guardado; si difiere, revierte eventos huérfanos y reprocesa.
- **Índice secundario** por dirección participante.
- **Consultas** estilo `eth_getLogs` (rango, contrato, topic0, participante) con **paginación por cursor**.
- **Agregados**: saldos ERC-20 derivados de `Transfer`, listados activos del marketplace.
- **Checkpoint** JSON para reanudar.

```bash
cargo test   # 9 tests
cargo run    # demo con reorg
```

## Limitaciones
ABI simplificada (valores ≤ u128); store en memoria (reemplazable por SQLite/Postgres detrás de la misma API); la cadena es un mock.
