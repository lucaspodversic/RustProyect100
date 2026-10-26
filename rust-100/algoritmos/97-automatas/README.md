# 97 · Máquina de Turing y autómatas finitos

Este proyecto es un simulador educativo, sin dependencias, que cubre dos partes:

- **Máquina de Turing** (`src/turing.rs`): una cinta infinita en ambos sentidos (`VecDeque`), un formato de texto propio, traza paso a paso y un límite de pasos (el problema de la parada impide saber de antemano si terminará).
- **Autómatas finitos** (`src/finito.rs`):
  - AFD con estado sumidero implícito.
  - AFN con transiciones ε (clausura‑ε).
  - **Construcción de subconjuntos** (AFN → AFD).
  - **Minimización** de Moore por refinamiento de particiones.

## Formato `.tm`

```text
# comentario
inicio: q0
aceptar: si
rechazar: no
blanco: _
q0 a -> ira _ R      # estado símbolo -> nuevo escribe L|R|S
```

Errores que se detectan: líneas mal formadas (con su número de línea), falta de `inicio:` y transiciones duplicadas (no determinismo).

Máquinas incluidas en `maquinas/`:

| Archivo | Qué hace | Verificado en tests |
|---|---|---|
| `incremento_binario.tm` | N → N+1 con acarreo | n = 0..300 |
| `palindromo.tm` | palíndromos sobre {a,b} | todas las cadenas hasta longitud 8 |
| `castor3.tm` | castor afanoso de 3 estados | 6 unos en 14 pasos |

## CLI

```bash
cargo run -- mt incremento 1011 --traza
cargo run -- mt palindromo abba
cargo run -- mt castor3 --traza
cargo run -- mt maquinas/mi_maquina.tm entrada --max 5000
cargo run -- multiplo 5 1010        # AFD de divisibilidad
cargo run -- explosion 10           # AFN de 11 estados → AFD mínimo de 1024
```

Ejemplo de traza (el cabezal va entre corchetes):

```
    4  derecha   1  0  1  1 [_]
    5  acarreo   1  0  1 [1] _
    ...
resultado: Acepta  pasos: 8  cinta: "1100"
```

## Resultados interesantes que comprueban los tests

- **Explosión exponencial**: el lenguaje "el k‑ésimo símbolo desde el final es `a`" se reconoce con un AFN de k+1 estados, pero su AFD mínimo necesita **2ᵏ** estados. Se comprueba para k = 1..6.
- **El AFD "resto módulo k" no siempre es mínimo**:

  | k | Estados del AFD mínimo |
  |---|---|
  | 6 | 4 |
  | 8 | 4 |
  | 12 | 5 |
  | impar | k (ya es mínimo) |

- En todos los casos, AFN, AFD y AFD minimizado aceptan exactamente las mismas cadenas: se comprueban todas las cadenas hasta longitud 10.

## Pruebas

```bash
cargo test   # 7 unitarios + 6 de integración
```

## Limitaciones

- La máquina de Turing es determinista y de una sola cinta. Los estados se guardan como `String`, lo que resulta claro pero no es lo más rápido.
- No incluye un conversor de expresión regular a AFN (Thompson); sería una buena extensión.
