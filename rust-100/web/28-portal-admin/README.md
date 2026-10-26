# 28 · Portal de administración con autenticación y panel

Aplicación web **renderizada en el servidor** (Axum + SQLite) con login, sesiones, roles y auditoría. No necesita JavaScript.

## Qué demuestra
- **Argon2id** para contraseñas; mensaje de error genérico (no revela si el usuario existe).
- **Sesiones del lado del servidor**: cookie `sid` (`HttpOnly; SameSite=Strict`, `Secure` opcional) con un token de 256 bits; en la base se guarda **sólo su SHA-256**. TTL configurable, limpieza de vencidas y revocación al desactivar el usuario.
- **Extractor propio** (`impl FromRequestParts for Sesion`): un handler que recibe `Sesion` queda protegido automáticamente (sin sesión → redirige a `/login`).
- **CSRF por sesión** en todos los POST, comparado en tiempo constante.
- **Roles** `admin` / `editor` / `lector`; reglas de negocio: no te podés desactivar a vos mismo, no se puede quitar el último admin.
- **Panel**: métricas, tabla de usuarios con acciones, alta de usuarios, **auditoría** de los últimos 20 eventos; `Cache-Control: no-store`.
- Escape HTML de todo dato de usuario (anti-XSS).

## Arquitectura
`src/lib.rs`: `Estado` (pool, parámetros Argon2, **reloj inyectable**, TTL), `crear_pool` (tablas `usuarios`, `sesiones`, `auditoria`), extractor `Sesion`, handlers y plantillas HTML con `format!` + `esc`. `src/main.rs`: crea el admin inicial desde variables de entorno.

## Uso
```bash
ADMIN_USUARIO=admin ADMIN_CLAVE=secreto123 cargo run
# Abrir http://localhost:3000/login
# Variables: DATABASE_URL=sqlite://portal.db, PUERTO=3000, COOKIE_SEGURA=1 (detrás de HTTPS)
```

## Tests
`cargo test` → 1 unitario (escape, cookies, comparación en tiempo constante, tokens) + 3 de integración con SQLite en memoria, Argon2 liviano y reloj falso: protección de rutas, login fallido genérico y auditado, hash del token en la base, permisos por rol, CSRF (faltante / de otra sesión), validación y duplicados, último admin, desactivación con revocación, logout y expiración exacta por TTL.

## Limitaciones
- Sin límite de intentos de login (ver proyecto 30 para un rate limiter que se puede montar delante).
- Sin recuperación de contraseña ni 2FA; plantillas inline (en un proyecto real: Askama/Maud).
