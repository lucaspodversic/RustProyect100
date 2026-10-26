# 67 · Nodo P2P que sincroniza bloques

Nodo de una red blockchain que **descubre pares, difunde bloques (gossip) y se sincroniza** con la cadena más larga, sobre TCP en `localhost`.

## Diseño
| Módulo | Responsabilidad |
|---|---|
| `cadena` | Bloques con PoW (dificultad 2) y validación; génesis común a todos. |
| `protocolo` | Mensajes JSON, uno por línea: `Hola`, `PedirBloques`, `Bloques`, `NuevoBloque`, `PedirPares`, `Pares`. |
| `nodo` | Máquina de estados pura: `procesar(msg) -> Vec<msg>`. Maneja huérfanos, duplicados y bifurcaciones (regla de cadena más larga). |
| `red` | Servidor `TcpListener` multi-hilo + cliente `sincronizar_con` + `difundir`. |

Separar la lógica del transporte permite testear el protocolo entero en memoria (`sincronizar_en_memoria`) y además probarlo con sockets reales.

## Uso
```bash
cargo test        # 11 tests (memoria + TCP real en 127.0.0.1)
cargo run         # demo con 3 nodos locales
```

## Limitaciones
- Sin cifrado ni autenticación de pares; sin límite anti-DoS más allá de `MAX_LOTE`.
- Reorganizaciones profundas piden la cadena completa (simple pero ineficiente).
