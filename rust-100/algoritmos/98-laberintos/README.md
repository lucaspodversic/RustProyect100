# 98 · Resolución de laberintos: BFS, DFS, Dijkstra y A*

Este proyecto genera laberintos aleatorios, los resuelve con cuatro algoritmos y dibuja el camino en ASCII. No tiene dependencias.

## Módulos

| Archivo | Contenido |
|---|---|
| `src/rejilla.rs` | `Laberinto` con celdas Muro / Libre / **Barro (coste 5)**, parseo desde texto (`#`, `.`, `~`, `S`, `E`), vecinos en 4 direcciones y dibujo del camino. |
| `src/generador.rs` | **Backtracker recursivo** con pila explícita (genera laberintos *perfectos*, que son árboles), con opciones `ciclos` % y `barro` %. PRNG xorshift con semilla. |
| `src/busqueda.rs` | BFS (cola), DFS (pila), Dijkstra y A* (montículo con `Reverse`, borrado perezoso y desempate por mayor g). Validador de caminos. |

## Comparativa

| Algoritmo | Garantía |
|---|---|
| BFS | Camino con **menos pasos**; ignora el coste del barro. |
| DFS | Ninguna: devuelve el primer camino que encuentra. |
| Dijkstra | Camino de **menor coste**. |
| A* (Manhattan) | Menor coste, **expandiendo menos nodos** que Dijkstra. |

Salida real de `cargo run --release -- 20 8 7`:

```
algoritmo   pasos  coste  expandidos
Bfs            56     72         299   ← menos pasos, pero pisa barro
Dfs            76    104          81   ← pocos expandidos, camino malo
Dijkstra       64     68         297   ← óptimo en coste
AEstrella      64     68         174   ← mismo coste, ~40 % menos trabajo
```

Después se dibuja el laberinto con el camino de A*: `*` marca las casillas libres pisadas y `o` el barro pisado.

## Uso

```bash
cargo run --release -- 30 12 2026 --ciclos 10 --barro 10
cargo run --release -- --archivo mi_laberinto.txt
cargo test     # 7 unitarios + 3 de propiedades (cientos de laberintos aleatorios)
```

## Propiedades que se verifican

- Todos los caminos son válidos: tienen el inicio y el final correctos, son contiguos y no atraviesan muros. Además, su coste coincide con el que se informa.
- El coste de A* es igual al de Dijkstra, que es menor o igual que el de BFS y el de DFS.
- BFS nunca usa más pasos que DFS.
- En total, A* expande menos nodos que Dijkstra.
- En un laberinto perfecto (sin ciclos), los cuatro algoritmos devuelven **el mismo** camino.
- El laberinto generado es un árbol: aristas = nodos − 1.

## Limitaciones

- Movimiento en 4 direcciones (no hay diagonales).
- No incluye animación paso a paso de la búsqueda; sería una extensión natural.
