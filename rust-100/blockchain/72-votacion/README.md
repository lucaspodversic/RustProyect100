# 72 · Contrato inteligente de votación descentralizada

Contrato de votación modelado como máquina de estados determinista (cada método recibe `remitente` y `altura`, como `msg.sender` / `block.number`) que **emite eventos**.

## Funcionalidades
- Registro de votantes con **peso** (sólo el presidente, sólo en fase de registro).
- **Delegación transitiva** con detección de ciclos (inspirado en `Ballot.sol`).
- **Voto público** o **commit-reveal** (`sha256(propuesta‖sal‖votante)`; incluir al votante evita que otro copie el compromiso).
- **Fases por altura**: Registro → Votación → (Revelación) → Cerrada.
- **Quórum** porcentual y desempate determinista (menor índice).
- **Votos firmados ed25519** enviados por un relayer, con nonce anti-replay y dominio por id de contrato.

```bash
cargo test   # 11 tests
cargo run    # demo
```

## Limitaciones
Didáctico: no corre en una EVM real (ver proyecto 74 y la versión Solidity del lote anterior). Las direcciones son strings.
