# 17 · Acortador de URLs con redirección y estadísticas

## Qué demuestra
- Generación de códigos **base62** a partir del id autoincremental (sin colisiones) dentro de una **transacción** SQLx; alias personalizados opcionales.
- Redirección **307** + conteo atómico con `UPDATE … RETURNING`.
- Validación de URLs con el crate `url` (solo http/https, sin bucles al propio dominio) y de alias (charset, longitud, reservados).
- Estadísticas por enlace (clics, creado, último acceso) y borrado.

## Endpoints
| Método | Ruta | Descripción |
|---|---|---|
| POST | `/api/acortar` | `{"url": "...", "alias": "opcional"}` → 201 |
| GET | `/{codigo}` | 307 al destino (+1 clic) |
| GET | `/api/stats/{codigo}` | estadísticas |
| DELETE | `/api/{codigo}` | borrar |

## Uso
```bash
BASE_PUBLICA=http://localhost:3000 cargo run
curl -XPOST localhost:3000/api/acortar -H 'content-type: application/json' -d '{"url":"https://www.rust-lang.org"}'
curl -i localhost:3000/1001
```

## Tests
`cargo test`: base62 y validaciones, flujo acortar→redirigir×3→stats, alias duplicado (409) y reservado (422), 404 y borrado.

## Limitaciones
- Códigos secuenciales → predecibles (se pueden enumerar). Para enlaces privados convendría un código aleatorio largo.
- Analíticas mínimas; las detalladas (por día y referrer) están en el proyecto 24.
