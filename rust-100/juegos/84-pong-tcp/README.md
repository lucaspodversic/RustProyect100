# 84 · Pong multijugador local por TCP

Pong para dos jugadores en red local con **servidor autoritativo**.

- **Lógica pura** (`juego.rs`): paletas acotadas, el ángulo de rebote depende del punto de impacto, aceleración del 6 % por golpe, detección por **cruce de línea** (la pelota rápida no atraviesa la paleta), saques alternados y victoria a N puntos.
- **Protocolo de texto** por líneas (`HOLA`, `IN -1|0|1`, `ESTADO …`, `FIN`), con validación, límite de 256 bytes por línea y nombres saneados.
- **Servidor**: tick fijo; un hilo lector por cliente envía por `mpsc` al bucle principal. Los clientes sólo mandan **intenciones**, nunca posiciones (anti-trampa). Un tercero recibe `ERROR lleno`; si alguien abandona, gana el rival. La basura en el protocolo se ignora.
- **Cliente** con IA configurable y render ASCII.

```bash
cargo test                                  # 9 tests (3 por TCP real en 127.0.0.1:0)
cargo run -- local                          # servidor + 2 IA en el mismo proceso
cargo run -- servidor 127.0.0.1:7878 5      # terminal 1
cargo run -- cliente 127.0.0.1:7878 ana     # terminales 2 y 3
```

## Limitaciones
Pensado para LAN: sin predicción ni interpolación del lado del cliente, sin cifrado ni autenticación. El cliente incluido juega con IA; para jugar a mano hay que implementar otro `Controlador` que lea teclas.
