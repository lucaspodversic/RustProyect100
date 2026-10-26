# 24 · Acortador de URLs con analíticas

## Qué demuestra
- Modelo de **eventos** (`clics`: una fila por visita) + agregaciones SQL (`GROUP BY dia/referrer/dispositivo`, `COUNT(DISTINCT)`), índice compuesto.
- Serie diaria **rellena con ceros** (lista para graficar) y rango por defecto de 7 días.
- **Privacidad**: no se guarda la IP; el visitante único es `blake3(sal|ip|ua|día)` (no reversible ni rastreable entre días); del referrer solo se guarda el host.
- Clasificación de dispositivo por User-Agent; bots excluidos de las métricas.
- Redirección 302 (no 301, que el navegador cachea y dejaría de contar), registro del clic que no rompe la redirección si falla.
- Fechas civiles sin dependencias (algoritmo de Hinnant) y **reloj inyectable** para tests.

## Uso
```bash
SAL=algo-secreto cargo run
curl -XPOST localhost:3000/api/enlaces -H 'content-type: application/json' -d '{"url":"https://rust-lang.org","codigo":"rust"}'
curl -I localhost:3000/rust
curl 'localhost:3000/api/enlaces/rust/analiticas?desde=2026-10-01&hasta=2026-10-31'
```

## Tests
Unitarios (UA, referrer, fechas ida y vuelta, hash por día) e integración con reloj simulado en varios días: totales, únicos, bots, serie con ceros, top referrers/dispositivos, validaciones y conflictos.

## Limitaciones
- `X-Forwarded-For` se confía tal cual (en producción, solo detrás de un proxy propio).
- Detección de dispositivo heurística; sin autenticación para ver analíticas.
