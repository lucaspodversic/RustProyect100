# 86 · Estación meteorológica IoT (Rust `no_std`)

Estación que mide **temperatura y humedad** con un sensor **Sensirion SHT3x** por I2C, agrega
las lecturas (mín/máx/promedio + punto de rocío) y publica un resumen **JSON** por **Wi‑Fi**
con reintentos y *backoff* exponencial. Pensada para un **ESP32**, pero toda la lógica es
genérica sobre los traits de [`embedded-hal` 1.0](https://docs.rs/embedded-hal), así que se
prueba en la PC sin hardware.

## Estructura

| Archivo | Qué hace |
|---|---|
| `src/sht3x.rs` | Driver SHT3x: comando single‑shot `0x2400`, lectura de 6 bytes, CRC‑8 (poly 0x31), conversión en enteros (centésimas). |
| `src/estadisticas.rs` | Acumulador O(1) de mín/máx/promedio y punto de rocío aproximado. |
| `src/json.rs` | `BufferFijo<N>`: `core::fmt::Write` sobre un array, sin heap. |
| `src/red.rs` | Trait `Transporte` (el "Wi‑Fi") y `enviar_con_reintentos` (500 ms → 1 s → 2 s → tope). |
| `src/estacion.rs` | Orquestador `tick()`: mide, acumula y publica cada N muestras; si la red falla no se pierden datos. |
| `examples/simulacion_host.rs` | Simulación en PC con un sensor virtual y un "Wi‑Fi" que imprime por consola. |

## Probar en la PC

```bash
cargo test                          # 18 tests (I2C y delay con embedded-hal-mock)
cargo run --example simulacion_host # imprime mediciones y los POST simulados
```

Los tests usan `embedded_hal_mock::eh1::i2c::Mock`, que verifica **byte a byte** cada
transacción I2C esperada (comando de medición, lectura con CRC), y `CheckedDelay` para
comprobar los tiempos exactos del backoff.

## Llevarlo al hardware real (ESP32)

1. Instalar la toolchain Xtensa: `cargo install espup && espup install` y `cargo install espflash`.
2. Crear el firmware con la plantilla oficial: `cargo generate esp-rs/esp-template` (o `esp-generate`).
3. Agregar este crate como dependencia (`estacion-meteorologica = { path = "../86-estacion-meteorologica" }`).
4. En `main`, construir el I2C de `esp-hal` (SDA=GPIO21, SCL=GPIO22 típicamente) y pasarlo al driver:

```rust
let i2c = esp_hal::i2c::master::I2c::new(peripherals.I2C0, Default::default())
    .unwrap().with_sda(peripherals.GPIO21).with_scl(peripherals.GPIO22);
let sensor = estacion_meteorologica::estacion::sensor_por_defecto(i2c);
// Implementar `Transporte` sobre esp-wifi + embassy-net (HTTP POST o MQTT).
let mut est = Estacion::new(sensor, mi_wifi, cfg);
loop { est.tick(&mut delay); delay.delay_ms(10_000); }
```

5. Flashear y ver el monitor serie: `cargo run --release` (usa `espflash flash --monitor`).

Conexión del SHT31: VIN→3V3, GND→GND, SDA→GPIO21, SCL→GPIO22, ADDR→GND (dirección `0x44`).

## Limitaciones

- No se probó en un ESP32 físico (el entorno no tiene hardware); el Wi‑Fi está abstraído por un trait.
- El punto de rocío usa la aproximación lineal de Lawrence (error < 1 °C con HR > 50 %).
