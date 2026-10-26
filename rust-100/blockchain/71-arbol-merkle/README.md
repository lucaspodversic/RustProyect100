# 71 · Árbol de Merkle

Árbol de Merkle para **verificar la inclusión de transacciones** con pruebas de tamaño O(log n), en dos modos:

| Modo | Hojas | Nodos | Nivel impar | Uso |
|---|---|---|---|---|
| `Rfc6962` | `SHA256(0x00‖dato)` | `SHA256(0x01‖izq‖der)` | se promueve | recomendado (Certificate Transparency) |
| `Bitcoin` | `SHA256d(dato)` | `SHA256d(izq‖der)` | se duplica | compatible con bloques de Bitcoin |

Incluye: pruebas de inclusión, verificación, actualización de hoja en O(log n), cliente SPV y conversión de txids (orden invertido de Bitcoin).

## Tests destacados
- Raíz real del **bloque 100.000 de Bitcoin** a partir de sus 4 txids.
- Vectores RFC 6962 (árbol vacío y hoja vacía).
- Pruebas para todos los tamaños 1..33 en ambos modos.
- Demostración de **CVE-2012-2459** (duplicación en modo Bitcoin) y de que la separación de dominio frena la **segunda preimagen**.

```bash
cargo test
cargo run -- tx1 tx2 tx3
```
