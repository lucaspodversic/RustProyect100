# 83 · Reproductor de música en terminal

Reproductor TUI (ratatui) con el **audio detrás de un trait**, así se testea sin placa de sonido.

- `BackendAudio`: cargar, reproducir, pausar, detener, buscar, volumen, posición. Incluye **`BackendSimulado`**, un reloj determinista que registra cada llamada.
- Listas **M3U / M3U extendido** (`#EXTINF`), rutas relativas y duración desconocida (`-1`).
- Controles: play/pausa, detener, siguiente/anterior (si pasaron más de 3 s, "anterior" reinicia la pista), seek acotado, volumen 0–100 y mute que recuerda el nivel.
- **Aleatorio** con Fisher–Yates, manteniendo la pista actual; **repetir** no/todo/una. Los archivos rotos se saltan sin ciclar infinito.

| Tecla | Acción |
|---|---|
| espacio / p | play-pausa |
| n / b | siguiente / anterior |
| h / l, ← / → | −5 s / +5 s |
| + / − / m | volumen / mute |
| z / r / s / q | aleatorio / repetir / detener / salir |

```bash
cargo test                    # 10 tests
cargo run                     # interactivo (lista de demo)
cargo run -- mi.m3u --demo    # guion automático, imprime la UI
```

## Limitaciones
No decodifica audio real: el backend incluido es simulado. Para sonido real habría que implementar `BackendAudio` con `rodio` (no incluido, para no depender de ALSA en la compilación).
