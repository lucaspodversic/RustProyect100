# 95 · Ordenamiento paralelo con rayon

Este proyecto implementa **merge sort** y **quicksort** en versión secuencial y en versión paralela con `rayon::join`. Después se comparan con `sort`, `sort_unstable`, `par_sort` y `par_sort_unstable` en un benchmark propio.

## Estructura

| Archivo | Contenido |
|---|---|
| `src/secuencial.rs` | Inserción, `mezclar` (con buffer de n/2 y un atajo si ya está ordenado), merge sort *top‑down* estable, pivote "mediana de tres posiciones pseudoaleatorias" y partición en **tres vías** (Dijkstra). El quicksort recursa sobre la parte menor, así que la pila es O(log n). |
| `src/paralelo.rs` | `merge_sort_par`: cada mitad recibe su propia mitad del buffer mediante `split_at_mut`, sin `unsafe` ni locks. `quicksort_par`: tras particionar, ordena ambas zonas con `join`. `con_hilos(n, f)` permite medir el escalado con un número fijo de hilos. |
| `src/datos.rs` | Generadores de entrada: aleatoria, ordenada, inversa y pocas claves. |
| `tests/correccion.rs` | Comparación con `sort` para 8 tamaños × 4 distribuciones, prueba de **estabilidad** y prueba con un solo hilo. |
| `benches/comparativa.rs` | Benchmark con `Instant`: mediana de 5 repeticiones. |

## Uso

```bash
cargo test
cargo bench                 # n = 2 000 000
cargo bench -- 10000000     # n personalizado
cargo run --release --example demo
```

## Resultados (8 hilos, n = 2 M, `u64`)

| Algoritmo | Aleatoria | Ordenada | Pocas claves |
|---|---|---|---|
| quicksort (sec) | 170 ms | 42 ms | 31 ms |
| merge_sort (sec) | 164 ms | 1,4 ms | 68 ms |
| **quicksort_par** | **34 ms** | 11 ms | 23 ms |
| **merge_sort_par** | **41 ms** | 0,6 ms | 21 ms |
| std `sort_unstable` | 33 ms | 1,2 ms | 8 ms |
| rayon `par_sort_unstable` | 9 ms | 0,9 ms | 4 ms |

Escalado de `quicksort_par` (datos aleatorios):

| Hilos | Speedup |
|---|---|
| 1 | 1,0× |
| 2 | 1,8× |
| 4 | 3,3× |
| 8 | 4,5× |

El escalado no es lineal porque la partición de la raíz (y la mezcla final en el caso del merge sort) es **secuencial** y ocupa O(n). Es la ley de Amdahl.

## Lo que se aprende

1. **Umbral de paralelismo** (`UMBRAL_PARALELO = 4096`): crear tareas para trozos pequeños cuesta más de lo que se gana.
2. **Elección del pivote**: con la mediana de (primero, medio, último), la partición en tres vías deja patrones tipo "órgano" en los que el quicksort se degradaba a **1,1 s** con la entrada ya ordenada. Con posiciones pseudoaleatorias el tiempo baja a 42 ms. El comentario en el código explica por qué.
3. **Seguridad**: `split_at_mut` demuestra al compilador que las tareas trabajan sobre memoria disjunta, así que no puede haber *data races*.
4. **La std es muy buena**: `sort_unstable` (pdqsort / ipnsort) y `sort` (driftsort) detectan secuencias ya ordenadas, usan particiones sin saltos y ordenamiento por bloques. Superarlas no es el objetivo; el objetivo es entender el patrón.

## Limitaciones

- La mezcla y la partición de nivel superior son secuenciales. Una mezcla paralela por búsqueda binaria mejoraría el escalado.
- Solo funciona con `T: Copy`, para simplificar el buffer.
- Las cifras dependen de la máquina: los números de arriba se obtuvieron en una VM de 8 vCPU.
