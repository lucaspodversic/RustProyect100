# 21 · API To-Do (listas, ítems, completado)

## Qué demuestra
- Recursos **anidados** (`/listas/{id}/items/{item}`) con extracción de tuplas `Path<(i64, i64)>` y verificación de pertenencia (no se puede tocar un ítem desde otra lista).
- **PATCH parcial** con `Option<T>` + `COALESCE` y el patrón `Option<Option<T>>` para distinguir *ausente* / *null* / *valor*.
- Conteos agregados (`LEFT JOIN` + `SUM(CASE…)`), filtros opcionales en SQL (`? IS NULL OR …`), operación masiva (`completar-todo`).
- Validación de fechas a mano (bisiestos incluidos), `completado_en` automático, borrado en cascada.

## Uso
```bash
cargo run
curl -XPOST localhost:3000/listas -H 'content-type: application/json' -d '{"nombre":"Súper"}'
curl -XPOST localhost:3000/listas/1/items -H 'content-type: application/json' -d '{"titulo":"Yerba","vence":"2026-11-01"}'
curl -XPATCH localhost:3000/listas/1/items/1 -H 'content-type: application/json' -d '{"completado":true}'
```

## Tests
`cargo test`: fechas, flujo listas/ítems/conteos/filtros, completar y des-completar, PATCH de `vence` en sus tres estados, aislamiento entre listas y cascada.

## Limitaciones
- Sin usuarios (todas las listas son globales) ni orden manual de ítems.
