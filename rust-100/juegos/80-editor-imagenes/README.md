# 80 · Editor de imágenes

Editor por línea de comandos con **filtros hechos a mano** (el crate `image` sólo lee/escribe PNG).

| Comando | Efecto |
|---|---|
| `grises`, `invertir`, `sepia` | color |
| `brillo:N`, `contraste:K`, `umbral:T` | ajustes |
| `blur:SIGMA` | desenfoque gaussiano **separable** |
| `enfocar`, `bordes` | convolución 3×3 y Sobel |
| `recortar:X,Y,W,H` | recorte (valida límites, sin overflow) |
| `resize:W,H` / `resize-px:W,H` | bilineal / vecino más cercano |
| `rotar`, `espejo-h`, `espejo-v` | geometría |
| `deshacer`, `rehacer` | historial (máx. 20 pasos) |

```bash
cargo test                                            # 10 tests
cargo run                                             # demo → /tmp/editor-demo.png
cargo run -- foto.png salida.png grises blur:2 resize:800,600
```

Una operación inválida no modifica la imagen ni el historial. Los argumentos se acotan (p. ej. `blur` ≤ 50) para evitar cuelgues.

## Limitaciones
Sólo PNG (para JPEG habilitar la feature `jpeg` de `image`). Sin interfaz gráfica; la imagen entera vive en memoria.
