# 18 · Autenticación JWT (registro, login, rutas protegidas)

## Qué demuestra
- Hash de contraseñas con **Argon2id** (sal aleatoria por usuario; parámetros configurables).
- Emisión/verificación de **JWT HS256** (`jsonwebtoken`), con `exp` y sin tolerancia.
- **Extractores personalizados** de Axum (`FromRequestParts`): `Usuario` (token válido) y `Admin` (además rol `admin`). Si fallan, el handler no corre (401/403 + `WWW-Authenticate: Bearer`).
- Buenas prácticas: mismo error para email inexistente o clave incorrecta, emails únicos case-insensitive, nunca exponer hashes, secreto obligatorio ≥ 32 bytes.

## Endpoints
| Método | Ruta | Auth |
|---|---|---|
| POST | `/auth/registro` | — (el primer usuario es admin) |
| POST | `/auth/login` | — → `{"token": ...}` |
| GET | `/yo` | Bearer |
| GET | `/admin/usuarios` | Bearer + rol admin |

## Uso
```bash
JWT_SECRETO=$(openssl rand -hex 32) cargo run
curl -XPOST localhost:3000/auth/registro -H 'content-type: application/json' -d '{"email":"ana@x.com","clave":"clave-segura"}'
TOKEN=$(curl -s -XPOST localhost:3000/auth/login -H 'content-type: application/json' -d '{"email":"ana@x.com","clave":"clave-segura"}' | jq -r .token)
curl localhost:3000/yo -H "authorization: Bearer $TOKEN"
```

## Tests
`cargo test`: flujo completo, tokens ausentes/basura/firmados con otro secreto, mensajes indistinguibles en login, expiración, validaciones, duplicados case-insensitive y control de roles. Argon2 usa parámetros mínimos en tests para velocidad.

## Limitaciones
- Sin refresh tokens ni revocación (JWT stateless: un token robado vale hasta `exp`).
- Sin rate limiting en login (ver proyecto 30) ni verificación de email.
- Se incluye `Cargo.lock` con `simple_asn1 0.6.3` / `time 0.3.41` fijados para compilar con rustc 1.85 (versiones más nuevas de `time` exigen 1.88). Con un toolchain reciente podés hacer `cargo update`.
