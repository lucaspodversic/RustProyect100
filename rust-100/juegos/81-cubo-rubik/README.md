# 81 · Cubo de Rubik

Modelo completo de un 3×3 en terminal.

- **54 stickers**; las permutaciones de cada giro **se derivan de geometría 3D** (rotación de Rodrigues sobre vectores enteros), no hay tablas escritas a mano.
- Notación Singmaster: `U R F D L B`, capas medias `M E S`, rotaciones `x y z`, modificadores `'` y `2`.
- Inversa de secuencias, **orden** (p. ej. `R U` = 105, *sexy move* = 6), mezclas reproducibles sin redundancias.
- **Resolvedor IDDFS** con poda (misma cara / caras opuestas) para mezclas cortas.
- Red desplegada en texto o con colores ANSI.

```bash
cargo test                       # 11 tests
cargo run                        # mezcla de 5 y la resuelve
cargo run -- "R U R' U'"         # aplica y muestra orden e inversa
```

## Limitaciones
El resolvedor es exhaustivo: práctico hasta ~6 movimientos. Para cubos arbitrarios haría falta el algoritmo de dos fases de Kociemba con tablas de poda.
