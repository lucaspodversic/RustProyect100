# 29 · API de reservas (disponibilidad y conflictos)

API REST (Axum + SQLite) para reservar recursos (salas, canchas, consultorios) con horario de apertura, granularidad y detección de solapamientos.

## Qué demuestra
- **Intervalos semiabiertos** `[inicio, fin)`: 10–11 y 11–12 no chocan. Predicado `a.inicio < b.fin && b.inicio < a.fin` en Rust y en SQL.
- **Sin carreras**: comprobación + inserción en **una sola sentencia** `INSERT … SELECT … WHERE NOT EXISTS (…) RETURNING id`. Un test lanza 20 reservas simultáneas y sólo gana una.
- **409 informativo** con la lista de reservas en conflicto.
- **Disponibilidad**: huecos libres del día (fusionando ocupados) e inicios posibles alineados a la granularidad para una duración dada; no ofrece horarios pasados.
- Fechas sin dependencias externas (`src/agenda.rs`: algoritmo civil de Hinnant, validación de 29/02, etc.).
- Reglas: dentro del horario, alineada a la granularidad, ≤ 8 h, no en el pasado; cancelación lógica (se conserva el historial).

## Arquitectura
- `src/agenda.rs`: lógica pura (parseo/formateo, solapamiento, huecos, inicios posibles).
- `src/lib.rs`: `Estado` (pool + **reloj inyectable**), esquema, `ErrorApi` → JSON, handlers.

## Uso
```bash
cargo run
curl -XPOST localhost:3000/recursos -H 'content-type: application/json' \
  -d '{"nombre":"Sala A","apertura":"09:00","cierre":"18:00","granularidad_min":30}'
curl 'localhost:3000/recursos/1/disponibilidad?fecha=2026-10-27&duracion=60'
curl -XPOST localhost:3000/reservas -H 'content-type: application/json' \
  -d '{"recurso_id":1,"inicio":"2026-10-27T10:00","fin":"2026-10-27T11:00","titular":"Ana"}'
curl -XDELETE localhost:3000/reservas/1
```

## Tests
`cargo test` → 2 unitarios (fechas ida/vuelta e inválidas; solapamiento, huecos, inicios) + 4 de integración (conflictos y bordes, concurrencia, disponibilidad, validaciones).

## Limitaciones
- Zona horaria única implícita (la del negocio); no maneja cambios de horario de verano ni reservas que crucen la medianoche.
- En tests el pool en memoria tiene 1 conexión; la atomicidad igual la garantiza la sentencia única (en archivo con varias conexiones también, por el bloqueo de escritura de SQLite). En PostgreSQL se usaría una restricción `EXCLUDE USING gist`.
- Sin autenticación ni reservas recurrentes.
