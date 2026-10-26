# 76 · Shooter 2D

Shooter espacial con **motor propio headless**: paso fijo (30 Hz), entrada abstracta, oleadas progresivas, disparos enemigos, colisiones AABB, vidas con invulnerabilidad temporal y puntaje. El render escribe en un **framebuffer** (texto plano o ANSI truecolor) ⇒ todo se testea sin GPU ni ventana.

```bash
cargo test                 # 8 tests
cargo run -- 20            # el piloto automático juega 20 s
cargo run -- 20 --ansi     # con colores
```

## Frontend gráfico opcional
No se incluye Bevy/macroquad (requieren display/GPU y alargan mucho la compilación). La lógica está desacoplada: un frontend sólo debe mapear teclas → `Entrada`, llamar `actualizar()` cada `DT` y dibujar `enemigos/balas/jugador`.

## Limitaciones
Física simple (sin rotaciones), sin sonido.
