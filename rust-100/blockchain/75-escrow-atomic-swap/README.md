# 75 · Escrow con atomic swap entre dos partes

Intercambio **atómico cross-chain** mediante **HTLC** (Hashed TimeLock Contracts) sobre dos cadenas simuladas, más un **escrow 2-de-3 con árbitro**.

## Protocolo
1. Alicia genera secreto `s`, publica `H = sha256(s)`.
2. Alicia bloquea en A para Beto: hashlock `H`, timelock **T1** (largo).
3. Beto **verifica** (receptor, monto, estado, margen de tiempo) y bloquea en B para Alicia con `H` y **T2 < T1 − margen**.
4. Alicia reclama en B → revela `s` on-chain.
5. Beto lee `s` en B y reclama en A.

O cobran ambos, o ambos se reembolsan tras los timelocks. Se verifica conservación de fondos (`oferta_total`).

```bash
cargo test   # 8 tests: camino feliz, abandonos, reclamo tardío, HTLC tramposo, escrow 2-de-3
cargo run
```

## Limitaciones
Las cadenas son simuladas y avanzan al mismo ritmo; en la realidad hay que considerar tiempos de confirmación distintos (y en Bitcoin se usa `OP_SHA256 … OP_CHECKLOCKTIMEVERIFY`).
