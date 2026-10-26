# 20 · Chat WebSocket con salas e historial

## Qué demuestra
- WebSockets en **Axum** (`WebSocketUpgrade`, `split` en lector/escritor, `tokio::select!` para cortar ambos).
- `tokio::sync::broadcast` por sala (difusión a N clientes; los lentos pierden mensajes viejos en vez de bloquear a todos: `Lagged`).
- Historial acotado (`VecDeque`) enviado al unirse, salas creadas/destruidas bajo demanda, nombres únicos por sala.
- Protocolo JSON etiquetado (`{"tipo": "mensaje" | "entro" | "salio" | "historial" | "error"}`).
- Protección: validación de nombre/sala, `max_message_size`, recorte de texto, cliente HTML que usa `textContent` (sin XSS).

## Uso
```bash
cargo run      # abrir http://localhost:3000 en dos pestañas
websocat 'ws://localhost:3000/ws/general?nombre=ana'   # o con websocat
```

## Tests
- Unitarios (`salas.rs`): historial acotado, difusión, nombres únicos, limpieza de salas vacías.
- Integración (`tests/ws.rs`): servidor real en puerto efímero + clientes `tokio-tungstenite`: conversación, historial, entradas/salidas, aislamiento entre salas, nombre duplicado, parámetros inválidos y `/salas`.

## Limitaciones
- Estado en memoria de un solo proceso (para escalar horizontalmente: Redis pub/sub).
- Historial no persistente; sin autenticación (cualquiera elige su nombre).
