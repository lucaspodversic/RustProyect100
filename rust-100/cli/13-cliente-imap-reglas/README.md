# 13 · Cliente IMAP con reglas de filtrado

`imapreglas` se conecta a un buzón IMAP y aplica reglas (mover, marcar leído, destacar, borrar) definidas en JSON.

## Qué demuestra
- Implementación **a mano** de un cliente IMAP4rev1 (RFC 3501): tags, respuestas no etiquetadas, **literales `{n}`**, `UID SEARCH/FETCH/STORE/COPY`, `EXPUNGE`.
- Cliente genérico sobre `Read + Write` → testeable y combinable con TLS externo.
- Motor de reglas puro (`reglas.rs`), primera coincidencia gana, condiciones AND, reglas vacías no aplican (seguridad).
- **Servidor IMAP simulado** en memoria (`servidor_simulado.rs`) para tests y demo: sin servidor real.
- Modo simulación (`-n`) y credenciales por variables de entorno (`IMAP_USUARIO`, `IMAP_CLAVE`).

## Uso
```bash
# Demo completa contra el servidor simulado:
cargo run -- --demo -r reglas.ejemplo.json -n
cargo run -- --demo -r reglas.ejemplo.json
# Servidor real (texto plano; para 993/TLS usar un túnel como stunnel):
IMAP_USUARIO=yo IMAP_CLAVE=... cargo run -- -r reglas.json --servidor 127.0.0.1:1143
```

## Tests
`cargo test`: 7 tests de integración cliente ↔ servidor simulado por TCP local (login fallido, literales UTF-8, simulación sin cambios, aplicación real de reglas, UNSEEN, carpeta inexistente, cabeceras plegadas, motor de reglas).

## Limitaciones
- **Sin TLS** integrado (se dejó fuera para mantener dependencias livianas); usar túnel o envolver el stream con `rustls`/`native-tls`.
- No decodifica encoded-words MIME (`=?UTF-8?B?…?=`) en asuntos.
- Mover usa COPY + `\Deleted` + EXPUNGE (portátil, pero el EXPUNGE elimina también otros mensajes ya marcados `\Deleted`).
- El servidor simulado implementa solo lo que usa el cliente.
