# 69 · Explorador de bloques en TUI

Explorador de bloques para terminal hecho con **ratatui**: lista de bloques, detalle con transacciones, detalle de tx, historial por dirección y búsqueda por altura, hash (prefijo) o `@dirección`.

## Arquitectura
- `datos.rs` — trait `FuenteCadena` (lo que la UI necesita) + `CadenaSimulada` determinista. Para conectarlo a una red real (SOON, Solana, EVM) basta implementar el trait con un cliente RPC.
- `app.rs` — máquina de estados de vistas y teclas abstractas (`Tecla`), sin terminal.
- `ui.rs` — dibujo con ratatui; `renderizar_texto` usa `TestBackend` para obtener la pantalla como texto.

## Teclas
`↑/↓` mover · `PgUp/PgDn` saltar 10 · `Enter` abrir · `←/→` (o `h/l`) bloque anterior/siguiente · `/` buscar · `a` historial del emisor · `Esc` volver · `q` salir.

## Uso
```bash
cargo test                  # 10 tests sin TTY
cargo run                   # TUI interactiva
cargo run -- --snapshot     # captura en texto (sin terminal)
```

## Limitaciones
Datos simulados; el saldo de una dirección es neto de las txs vistas (no hay estado inicial).
