# 65 · RPC binario (bincode)

Framing length-prefix + bincode. Servidor registra handlers por nombre; cliente `call`.

## Limitaciones
Un request a la vez por conexión; sin timeouts ni versionado de esquema.
