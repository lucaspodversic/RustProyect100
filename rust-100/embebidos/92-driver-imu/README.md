# 92 · Driver del IMU MPU‑6050 con `embedded-hal` 1.0

Este es un driver `no_std` del acelerómetro y giroscopio **MPU‑6050**. Es genérico sobre `embedded_hal::i2c::I2c`, así que funciona con cualquier HAL: `esp-hal`, `rp2040-hal`, `embassy-*`, `stm32*-hal`, etc.

## Características

- Verifica la identidad del chip con `WHO_AM_I` (0x68) y devuelve `Error::IdIncorrecto` si no coincide.
- Hace un reset, lo despierta (al arrancar está en *sleep*) y selecciona el reloj PLL.
- Permite configurar los rangos (±2…16 g y ±250…2000 °/s), el filtro DLPF y el divisor de muestreo.
- Lee los 14 bytes en ráfaga con `write_read` (*repeated start*), de modo que todos los datos son coherentes entre sí.
- Convierte a unidades con enteros: mili‑g, mili‑°/s y centésimas de °C.
- Calibra el *bias* del giroscopio promediando lecturas con el sensor quieto.

## Módulos

| Archivo | Contenido |
|---|---|
| `src/registros.rs` | Direcciones de registros, `RangoAcel`, `RangoGiro`, `Dlpf`. |
| `src/mpu6050.rs` | `Mpu6050<I>`, `Datos`, `Error<E>` y tests con `embedded-hal-mock`. |
| `examples/simulador.rs` | Un chip simulado (banco de registros con auto‑incremento) que hace de esclavo I2C. Calcula el ángulo de inclinación. |

## Uso

```rust
let mut mpu = Mpu6050::new(i2c, registros::DIR_DEFECTO);
mpu.iniciar(&mut delay, RangoAcel::G4, RangoGiro::Dps500, Dlpf::Hz44, 4)?;
mpu.calibrar_giro(&mut delay, 500)?;          // sensor quieto
let d = mpu.leer_crudo()?;
let [ax, ay, az] = mpu.acel_mg(&d);
```

## Probar

```bash
cargo test                       # 6 tests: verifican byte a byte cada transacción I2C
cargo run --example simulador
```

## Conexión real

| MPU‑6050 | Pico (RP2040) | ESP32 |
|---|---|---|
| VCC | 3V3 | 3V3 |
| GND | GND | GND |
| SDA | GP4 | GPIO21 |
| SCL | GP5 | GPIO22 |
| AD0 | GND (dirección 0x68) | GND |

Los módulos GY‑521 ya traen pull‑ups de 4,7 kΩ. Usa el bus a 400 kHz.

Para flashear: con Pico, `cargo run --release --target thumbv6m-none-eabi` (runner `probe-rs` o `elf2uf2-rs`); con ESP32, `espflash flash --monitor`.

## Limitaciones

- No soporta FIFO, interrupciones, DMP ni el magnetómetro auxiliar del MPU‑9250.
- No está probado con un chip físico.
