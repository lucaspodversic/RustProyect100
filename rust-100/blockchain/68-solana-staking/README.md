# 68 · Smart contract de staking estilo Solana

Programa de **staking de SOL con recompensas por tiempo**, escrito sobre un mini-runtime que reproduce el modelo de cuentas de Solana.

## Por qué no Anchor directo
Instalar `anchor-cli` + `solana-cli` + toolchain SBF requiere varias descargas grandes y un validador local; para que `cargo test` funcione en cualquier máquina, el programa vive en `src/` como lógica Rust pura. La **versión Anchor equivalente** está en `anchor/` (no forma parte del build del crate).

### Ruta Anchor
```bash
sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"
cargo install --git https://github.com/coral-xyz/anchor avm && avm install latest && avm use latest
cd anchor && anchor init . --force   # o copiar programs/staking a un workspace nuevo
anchor build && anchor test
```

## Qué modela
- **Cuentas** con `owner`, lamports y datos; **PDAs** con bump canónico (`["pool", autoridad]`, `["stake", pool, usuario]`, `["boveda", pool]`, `["tesoro", pool]`).
- **Discriminador** de 8 bytes por tipo de cuenta (como Anchor).
- Instrucciones serializadas en binario (tag + args LE).
- **Atomicidad**: la tx se ejecuta sobre una copia y solo se confirma si no hay error.
- **Conservación de lamports** verificada tras cada instrucción.
- Recompensas en punto fijo (`ESCALA = 1e9`), período de bloqueo, pausa por autoridad, tesoro fondeable.

## Ataques cubiertos por tests
Falta de firma, sustitución de cuentas (posición/bóveda ajena), *type cosplay* (cuenta de otro tipo o de otro dueño), doble inicialización, retiro anticipado, retiro excesivo.

```bash
cargo test     # 15 tests
cargo run      # demo
```

## Limitaciones
Didáctico y sin auditar. Las PDAs usan un criterio de bump simulado (no verifican "off-curve" real). Sin CPI a SPL Token (se stakea SOL nativo).
