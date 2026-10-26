# 77 · Snake con high score persistente

- **Lógica pura** (`juego.rs`): cola de giros (no se pierden teclas rápidas), giro de 180° bloqueado, modos **paredes** o **toroide**, comida uniforme en celdas libres, perseguir la cola es legal, aceleración por puntaje, condición de victoria.
- **IA** (`ia.rs`): BFS hacia la comida + flood fill para no encerrarse.
- **Récords** (`records.rs`): top-10 en JSON (`$XDG_DATA_HOME/snake/records.json`), **escritura atómica** (tmp + rename), nombres saneados y respaldo automático si el archivo está corrupto.
- **Terminal** (`main.rs`, crossterm): flechas/WASD, `q` sale.

```bash
cargo test                       # 9 tests
cargo run                        # jugar (necesita TTY)
cargo run -- --auto --records /tmp/r.json   # juega la IA
```

## Limitaciones
El modo interactivo necesita una terminal real; sin TTY juega la IA automáticamente.
