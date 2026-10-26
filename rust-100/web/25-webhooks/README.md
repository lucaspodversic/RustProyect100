# 25 · Webhooks: recibir y reenviar

## Qué demuestra
- **Verificación HMAC-SHA256** sobre los bytes crudos (`"{ts}.{cuerpo}"`), comparación en tiempo constante y **anti-replay** con tolerancia de tiempo.
- **Idempotencia** por `(fuente, X-Evento-Id)` con `INSERT … ON CONFLICT DO NOTHING RETURNING`.
- Patrón *outbox*: se responde **202** al instante y un worker reenvía desde una cola **durable en SQLite**.
- Reintentos con **backoff exponencial**, *dead letter* (`fallida`) tras N intentos y reintento manual.
- Reenvío **firmado** con un secreto propio (el destino puede verificarnos).
- Límite de tamaño de cuerpo, reloj inyectable para testear tiempos sin `sleep`.

## Uso
```bash
FUENTES="pagos=s3cr3t" DESTINOS="pagos=http://localhost:9000/hook" cargo run
# Firmar a mano:
TS=$(date +%s); BODY='{"monto":10}'
FIRMA="sha256=$(printf '%s.%s' "$TS" "$BODY" | openssl dgst -sha256 -hmac s3cr3t -hex | cut -d' ' -f2)"
curl -XPOST localhost:3000/webhooks/pagos -H "x-timestamp: $TS" -H "x-firma: $FIRMA" -H 'x-evento-id: ev1' -d "$BODY"
curl localhost:3000/eventos/1/entregas
```

## Tests
- `firma.rs`: firma válida, secreto/cuerpo/ts alterados, fuera de tiempo, formato.
- `tests/webhooks.rs` (destinos HTTP locales reales): verificación, duplicados, replay, headers faltantes, fuente desconocida, 413; reenvío a dos destinos con uno que falla una vez, respeto del backoff, firma de salida verificable; dead letter tras 3 intentos y reintento manual.

## Limitaciones
- Un solo worker (con varios habría que reclamar filas atómicamente).
- Sin rotación de secretos ni UI; los destinos se configuran por variables de entorno.
