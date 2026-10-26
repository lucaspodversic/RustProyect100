# 94 · Estructuras de datos desde cero

Este crate implementa tres estructuras clásicas en **Rust 100 % seguro** (`#![forbid(unsafe_code)]`) y sin dependencias:

| Estructura | Archivo | Detalles |
|---|---|---|
| **Lista enlazada** | `src/lista.rs` | `Option<Box<Nodo>>`, push/pop/peek, `invertir` en el sitio, eliminar con cursor `&mut`, `Iter`/`IterMut`/`IntoIter`, `FromIterator` y **Drop iterativo** (1 millón de nodos sin desbordar la pila). |
| **Árbol binario de búsqueda** | `src/arbol.rs` | Mapa ordenado `K→V`: insertar, obtener, eliminar (hoja, un hijo o dos hijos con sucesor), mín/máx, altura y recorrido en orden iterativo. Deliberadamente **sin balanceo**, para mostrar la degeneración con claves ordenadas. |
| **Tabla hash** | `src/tabla_hash.rs` | Encadenamiento separado, hasher **FNV‑1a** (verificado con los vectores oficiales), capacidad en potencias de 2, rehash con carga > 0,75, `swap_remove` y `obtener_o_insertar` (tipo `entry`). |

Además, `src/xorshift.rs` contiene un PRNG mínimo para los tests y ejemplos.

## Pruebas

```bash
cargo test
```

Hay 12 tests en total:

- **9 unitarios**: casos concretos y límites de cada estructura.
- **3 diferenciales** (`tests/contra_std.rs`): aplican miles de operaciones aleatorias a cada estructura y a su equivalente de la std (`BTreeMap`, `HashMap`, `Vec`) y comprueban que los resultados coinciden en cada paso.

## Comparar rendimiento

```bash
cargo run --release --example comparar
```

Resultado de ejemplo con 200 000 claves `u64` aleatorias:

| Operación | Nuestra | std |
|---|---|---|
| Hash: insertar | ~47 ms | ~4 ms |
| Hash: buscar | ~7 ms | ~3 ms |
| BST: insertar | ~65 ms | ~10 ms (BTreeMap) |
| BST: buscar | ~70 ms | ~20 ms |

**Por qué la std gana:**

- `HashMap` usa *SwissTable*: direccionamiento abierto con metadatos SIMD y una sola reserva de memoria. Nuestra tabla crea un `Vec` por cubeta y vuelve a reservar memoria en cada rehash.
- `BTreeMap` guarda muchas claves por nodo, lo que aprovecha mejor la caché. Nuestro BST usa un nodo por clave y no está balanceado: con estas claves la altura es 46, cuando lo ideal sería unas 18.

## Lecciones de *borrow checker*

- `Option::take()` y `mem::replace` permiten mover datos fuera de una estructura sin dejarla inválida.
- Un cursor `&mut Option<Box<Nodo>>` permite modificar "el enlace que apunta al nodo", sin necesidad de un puntero al padre.
- `as_deref()` / `as_deref_mut()` convierten `&Option<Box<T>>` en `Option<&T>`.
- En `IterMut` se usa `take()` sobre `Option<&mut>` para no tener dos referencias mutables vivas a la vez.

## Limitaciones

- El BST no se balancea (no es AVL ni rojo‑negro).
- La tabla hash no resiste HashDoS (FNV no usa semilla aleatoria) y no implementa la API `Entry` completa.
