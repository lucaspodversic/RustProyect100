# 74 · Máquina virtual EVM simplificada

Intérprete didáctico de la **Ethereum Virtual Machine** escrito desde cero (sin `revm`).

## Incluye
- `U256` propio (4×u64): suma/resta/multiplicación mod 2^256, división larga, exponenciación, shifts.
- ~60 opcodes: aritmética, comparación, bits, `KECCAK256`, `CALLER`, `CALLVALUE`, `CALLDATALOAD/SIZE`, memoria (`MLOAD/MSTORE/MSTORE8/MSIZE`), storage (`SLOAD/SSTORE`), `JUMP/JUMPI/JUMPDEST/PC/GAS`, `PUSH0..PUSH32`, `DUP1..16`, `SWAP1..16`, `LOG0..4`, `RETURN`, `REVERT`.
- **Gas**: costos base + expansión de memoria cuadrática + EXP/KECCAK/SSTORE/LOG dinámicos.
- **Semántica de errores**: REVERT devuelve datos y gas restante; errores excepcionales consumen todo el gas; storage/logs sólo se confirman en éxito.
- **Análisis de JUMPDEST** (no se puede saltar dentro de datos de PUSH).
- **Ensamblador** con etiquetas (`bucle:` / `@bucle`).

```bash
cargo test   # 12 tests (incluye keccak("") y 20! en bytecode)
cargo run    # factorial(10)
```

## Limitaciones
Sin `CALL/CREATE`, sin aritmética con signo (`SDIV`, `SLT`…), gas simplificado (no warm/cold de EIP-2929 ni reembolsos).
