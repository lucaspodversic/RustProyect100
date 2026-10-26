# 89 · Monitor de calidad del aire (Rust `no_std`)

Mide **partículas** (PM1.0/PM2.5/PM10 con un **PMS5003** por UART) y **CO₂** (con un
**SCD40** por I2C), calcula el **AQI de la EPA (revisión 2024)**, muestra todo en un
**LCD 16×2** (HD44780 con backpack I2C PCF8574) y activa una **alarma con histéresis**.

## Módulos

| Archivo | Contenido |
|---|---|
| `src/pms5003.rs` | Parser byte a byte de tramas de 32 bytes (`0x42 0x4D`, checksum), resincronización y lectura desde cualquier `embedded_io::Read`. |
| `src/scd40.rs` | Driver SCD40: start/stop periódico, *data ready*, lectura con CRC‑8 Sensirion. |
| `src/aqi.rs` | Tablas de cortes EPA para PM2.5/PM10 en décimas de µg/m³, categorías y niveles de CO₂. |
| `src/filtro.rs` | Media móvil O(1) con buffer circular. |
| `src/lcd.rs` | HD44780 en modo 4 bits vía PCF8574 (RS/RW/EN/BL + D4..D7). |
| `src/monitor.rs` | AQI combinado = máx(sub‑índices), alarma con histéresis (on ≥101, off <90; o CO₂ ≥1500), texto de 2×16. |
| `examples/simular.rs` | Flujo UART simulado con un pico de humo; imprime el LCD en la terminal. |

## Probar

```bash
cargo test                  # 13 tests (I2C/pines con embedded-hal-mock, UART con &[u8])
cargo run --example simular
```

## Hardware real (Raspberry Pi Pico o ESP32)

| Dispositivo | Conexión |
|---|---|
| PMS5003 (5 V) | TX del sensor → RX de la UART (GP1 en Pico), 9600 8N1 |
| SCD40 | I2C0: SDA GP4, SCL GP5, 3V3 |
| LCD 16×2 + PCF8574 (5 V) | mismo I2C (dirección 0x27); usar level shifter si el backpack tiene pull‑ups a 5 V |
| Buzzer/LED alarma | GP15 |

Las UART de `rp2040-hal`/`embassy-rp`/`esp-hal` implementan `embedded_io::Read`; el I2C
implementa `embedded_hal::i2c::I2c` y puede compartirse con `embedded-hal-bus`.
Flasheo en Pico: `cargo run --release --target thumbv6m-none-eabi` con `probe-rs` o
`elf2uf2-rs` como runner (ver proyecto 88). En ESP32: `espflash flash --monitor`.

## Limitaciones

- Sin probar con sensores físicos. El AQI oficial usa promedios de 24 h (o NowCast);
  aquí se aplica sobre una media móvil de 10 lecturas como indicador en tiempo real.
