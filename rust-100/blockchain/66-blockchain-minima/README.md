# 66 · Blockchain mínima (PoW + validación)

Cadena de bloques didáctica en Rust: bloques enlazados por hash SHA-256, **Prueba de Trabajo** con dificultad ajustable y **validación completa** de la cadena.

## Qué incluye
- `Bloque` con índice, timestamp, datos, hash previo, nonce, dificultad y hash.
- Serialización canónica del encabezado (enteros little-endian + prefijo de longitud) para evitar colisiones por ambigüedad.
- Minado por fuerza bruta del `nonce` hasta obtener N ceros hexadecimales.
- Ajuste de dificultad cada `ventana_ajuste` bloques según el tiempo real vs. objetivo.
- Validación: génesis, índices consecutivos, enlaces, hash recalculado, PoW y tiempo monótono.
- Regla de consenso "cadena más pesada" (`reemplazar_si_mejor`).

## Uso
```bash
cargo test            # 14 tests
cargo run -- 4 6      # dificultad 4, 6 bloques
```

## Limitaciones
Didáctico: los datos son texto libre (sin transacciones ni firmas; ver proyectos 70 y 71), y el trabajo acumulado es una aproximación `16^dificultad`.
