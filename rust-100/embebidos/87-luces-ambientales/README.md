# 87 · Controlador de luces ambientales (Rust `no_std`)

Controla una **tira LED RGB** (3 canales PWM) eligiendo el color según **la hora del día**
(curva circadiana), **la luz ambiente** (LDR por ADC) y **la presencia** (sensor PIR).
Hace fundidos suaves y corrige gamma para que los cambios se perciban parejos.

## Módulos

| Archivo | Contenido |
|---|---|
| `src/color.rs` | `Rgb`, `Hsv` (conversión entera por sectores), mezcla lineal, `acercar` (fundido) y `gamma16` (γ≈2). |
| `src/horario.rs` | Fotogramas clave (00:00 rojo tenue, 06:00 amanecer, 13:00 blanco frío, 19:00 ámbar, 22:00 rojo) con interpolación circular. |
| `src/sensores.rs` | Trait `SensorAnalogico` (embedded‑hal 1.0 no trae ADC), LDR→lux por tabla, lux→brillo. |
| `src/controlador.rs` | `Controlador` genérico sobre `SetDutyCycle` + `InputPin`: modos Auto/Manual/Apagado, apagado por ausencia, fallas seguras. |
| `examples/dia_completo.rs` | Imprime la curva del día como bloques de color en la terminal. |

## Probar

```bash
cargo test                       # 17 tests: PWM/PIR/LDR falsos + pin mock de embedded-hal-mock
cargo run --example dia_completo # ver la curva de colores en la terminal (colores ANSI 24 bits)
```

## Hardware real (Raspberry Pi Pico / RP2040 o ESP32)

- 3 MOSFET N (p. ej. IRLZ44N) para R, G, B de una tira 12 V; gates a GPIO 2/3/4 (PWM).
- PIR HC‑SR501 → GPIO 15 (entrada). LDR GL5528 + 10 kΩ en divisor → GPIO 26 (ADC0).
- Con `rp2040-hal`/`embassy-rp`: los canales PWM implementan `SetDutyCycle`, los pines
  `InputPin`; para el LDR implementar `SensorAnalogico` envolviendo el `Adc` del HAL.
- Hora real: un RTC (DS3231 por I2C) o NTP en el ESP32 → `minuto_dia`.
- Flasheo en Pico: `cargo install elf2uf2-rs`, compilar con `--target thumbv6m-none-eabi`
  y arrastrar el `.uf2` (o `probe-rs run` con una Debug Probe).

## Limitaciones

- Sin probar sobre hardware físico; la calibración LDR→lux es de ejemplo (GL5528 + 10 kΩ).
- Gamma 2,0 (cuadrado) en vez de 2,2 para evitar punto flotante.
