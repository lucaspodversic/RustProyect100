# 99 · Compresor RLE + Huffman canónico

Este es un compresor sin dependencias que combina dos técnicas:

- **RLE estilo PackBits**: comprime las rachas de bytes repetidos y en el peor caso solo crece 1 byte por cada 128.
- **Huffman canónico**: asigna códigos más cortos a los bytes más frecuentes.

Ambas van dentro de un formato contenedor con **CRC‑32** y selección automática del mejor método.

## Módulos

| Archivo | Contenido |
|---|---|
| `src/rle.rs` | PackBits: cabecera `n ≥ 0` indica n+1 literales; `n < 0` indica que el siguiente byte se repite 1‑n veces. Solo se codifican como repetición las rachas de 3 o más bytes. |
| `src/bits.rs` | `EscritorBits` (acumulador `u64`, MSB primero) y `LectorBits`. |
| `src/huffman.rs` | Frecuencias → árbol (montículo) → longitudes, con un **límite de 15 bits** (que se aplica re‑escalando frecuencias) → códigos **canónicos**. La decodificación compara rangos por longitud, sin reconstruir el árbol. Se valida la desigualdad de **Kraft**. |
| `src/crc32.rs` | CRC‑32 IEEE con tabla generada por `const fn` en tiempo de compilación. |
| `src/contenedor.rs` | Formato `CMP1`: mágico, método, tamaño, CRC y carga. `comprimir()` prueba los 4 métodos y elige el más pequeño. |

## Formato del archivo

```
"CMP1" | método u8 | tamaño original u64 LE | CRC-32 u32 LE | carga…
Carga Huffman: tamaño u64 LE | 256 longitudes u8 | flujo de bits
```

Los 256 bytes de longitudes son toda la "tabla" que se guarda. Esa es la ventaja de los códigos canónicos, y es la misma idea que usa DEFLATE.

## Uso

```bash
cargo run --release -- c entrada.txt salida.cmp          # método automático
cargo run --release -- c entrada.bmp salida.cmp --metodo rle+huffman
cargo run --release -- d salida.cmp restaurado.txt
cargo run --release -- info salida.cmp
cargo run --release -- comparar archivo
cargo test   # 8 unitarios + 4 de integración
```

## Resultados reales

**1,1 MB de código fuente Rust** (todos los proyectos de este lote):

| Método | Tamaño | Tiempo |
|---|---|---|
| Almacenado | 100 % | 3,6 ms |
| RLE | 89,6 % | 4,6 ms |
| **Huffman** | **62,0 %** | 7,2 ms |
| RLE+Huffman | 62,8 % | 8,8 ms |

**300 KB aleatorios**: RLE da 100,8 % y Huffman 100,1 %, así que el modo automático elige **Almacenado**, que solo añade los 17 bytes de cabecera.

**10 000 ceros**: menos de 400 bytes.

Como referencia, gzip deja el código fuente en ~20 %. La diferencia se debe a que Huffman solo explota las frecuencias de cada byte, mientras que gzip añade **LZ77**, que aprovecha las repeticiones de cadenas.

## Robustez (probada)

- Ida y vuelta en 8 entradas, que incluyen:
  - entrada vacía y un solo byte;
  - ceros, los 256 valores y datos aleatorios;
  - datos dispersos y texto.

  Se prueba con los 4 métodos.
- **Corrupción detectada**: se cambia un bit en decenas de posiciones de la carga útil y también se truncan archivos. Siempre se obtiene `Err`.
- **Fuzzing sencillo**: 2000 entradas basura con cabecera válida. Ninguna provoca *panic*, y el tamaño declarado no puede forzar una reserva enorme de memoria.
- El ejemplo del libro de Cormen produce las longitudes 1, 3, 3, 3, 4, 4 y los códigos canónicos esperados. Las frecuencias de Fibonacci activan el límite de 15 bits.

## Limitaciones

- No incluye LZ77/LZSS. Sería el siguiente paso hacia DEFLATE.
- El límite de longitud usa una heurística (re‑escalar frecuencias) en lugar del algoritmo óptimo *package‑merge*.
- La decodificación es bit a bit (simple, pero no la más rápida; no se ha medido su rendimiento). Una tabla de búsqueda de 15 bits la aceleraría.
- Todo el archivo se carga en memoria (no funciona en *streaming*).
