# 78 · Juego de plataformas con física y colisiones

Plataformero headless con **tilemap ASCII** y física "de juego" (no realista, sino agradable):

- Gravedad con velocidad terminal, aceleración/fricción horizontal.
- **Colisiones AABB por ejes** (X luego Y) contra tiles + **sub-pasos anti-tunneling**.
- **Coyote time**, **jump buffer** y **salto variable** (soltar corta el salto).
- **Plataformas de un sentido** (`=`): se atraviesan desde abajo; abajo+saltar para bajar.
- Monedas, pinchos, caída al vacío, vidas, meta y **cámara** que sigue al jugador.

```bash
cargo test   # 11 tests
cargo run    # replay headless con fotogramas en texto
```

## Limitaciones
Sin enemigos ni plataformas móviles; render en texto (un frontend gráfico sólo debe dibujar `nivel.tiles` y `jugador`).
