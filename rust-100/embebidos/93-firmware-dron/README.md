# 93 · Firmware básico de dron (cuadricóptero en X) + simulador

Este es el núcleo de un controlador de vuelo en `no_std` (con `libm` para las funciones trigonométricas). Incluye estimación de actitud, PID en cascada, mezclador X, salida PWM a los ESC y una máquina de seguridad. Todo se prueba **en lazo cerrado contra un simulador físico**, sin drone real.

```text
IMU ─► filtro complementario ─► PID ángulo ─► PID velocidad ─► mezclador X ─► ESC (PWM)
radio ─► setpoints (modo ángulo)        seguridad: armado / failsafe / vuelco
```

## Módulos

| Archivo | Contenido |
|---|---|
| `src/actitud.rs` | Ángulos a partir de la gravedad y filtro complementario (α = 0,98), que corrige la deriva del giroscopio. |
| `src/pid.rs` | PID con derivada sobre la medición, anti‑windup y salida saturada. |
| `src/mezclador.rs` | Tabla de mezcla X y desaturación que conserva las diferencias entre motores. |
| `src/motores.rs` | `Motores<P: SetDutyCycle>`: pulsos de 1000–2000 µs con frecuencia configurable. Se prueba con el mock de PWM. |
| `src/seguridad.rs` | Estados Desarmado/Armado/Failsafe, condiciones de armado, timeout de radio de 0,5 s, descenso de 5 s y desarme por vuelco (>70°). |
| `src/control.rs` | `Controlador::ciclo(imu, radio, dt)`: une todo. No integra con el acelerador bajo. |
| `src/simulacion.rs` | Dinámica rotacional con retardo de motor, inercia y amortiguamiento, más un IMU con ruido y bias (xorshift). |
| `tests/vuelo_simulado.rs` | Prueba que estabiliza tras una ráfaga, sigue setpoints de ángulo y yaw, entra en failsafe y no arma inclinado. |
| `examples/vuelo.rs` | Vuelo de 6 s con fases y tabla de telemetría. |

## Probar

```bash
cargo test                # 15 tests (11 unitarios + 4 de lazo cerrado)
cargo run --example vuelo
```

En la simulación, una ráfaga de 25° en roll se corrige a menos de 0,5° en unos 0,7 s. Un setpoint de 10° en pitch se alcanza en unos 0,4 s.

## Llevarlo a hardware (orientativo)

1. **Placa**: RP2040 o ESP32‑S3 con un MPU‑6050 (driver del proyecto 92) por I2C a 400 kHz.
2. **Lazo**: ejecútalo a 1 kHz con un timer o una tarea de `embassy`. Convierte las lecturas a g y °/s y llama a `Controlador::ciclo`.
3. **Radio**: usa un receptor SBUS/CRSF por UART y normaliza los canales a `Radio`. `enlace = false` si no llegan tramas o el receptor marca failsafe.
4. **ESC**: un canal PWM por motor, a 50 Hz (o 400 Hz si el ESC lo admite), a través de `Motores::escribir`.
5. **Seguridad de verdad**: **prueba siempre sin hélices** primero. Comprueba el sentido de giro de cada motor y la orientación del IMU antes de volar.

## Limitaciones

- Es un firmware **didáctico**. No tiene control de altura, GPS, magnetómetro, DShot, filtros notch ni ajuste fino de ganancias para un dron real. Las ganancias están ajustadas para el modelo simulado.
- El simulador solo modela la rotación (sin traslación, viento continuo ni efecto suelo).
- No se compiló para un target embebido (no hay `rustup` en el entorno de construcción).
