# 88 · Reloj despertador (Raspberry Pi Pico + OLED + botones) — Rust `no_std`

Despertador completo: hora, alarma activable, **snooze** de 5 minutos, auto‑apagado a los
10 minutos, ajuste con 3 botones (MODO / MÁS / SNOOZE‑MENOS), pantalla **OLED SSD1306
128×64** por I2C con dígitos grandes y buzzer con patrón intermitente.

## Módulos

| Archivo | Contenido |
|---|---|
| `src/hora.rs` | `Hora` como segundos desde medianoche, aritmética circular de 24 h. |
| `src/botones.rs` | Antirrebote por integrador, pulsación corta/larga sobre `InputPin` (activo en bajo). |
| `src/despertador.rs` | Máquina de estados Normal → AjusteReloj → AjusteAlarma → Sonando ⇄ Pospuesta; buzzer `OutputPin`. |
| `src/fuente.rs` | Fuente 5×7 por columnas (dígitos, mayúsculas, `:`). |
| `src/pantalla.rs` | Framebuffer 1 bpp en layout de páginas del SSD1306, texto escalable, inversión; driver SSD1306 (init + volcado en bloques de 16 bytes). |
| `src/interfaz.rs` | Compone la pantalla: título, hora grande centrada (escala 3), campo en edición resaltado, alarma abajo. |
| `examples/simular.rs` | Muestra la OLED como arte ASCII en la terminal y simula la alarma + snooze. |

## Probar

```bash
cargo test                  # 17 tests (I2C y pines con embedded-hal-mock)
cargo run --example simular # ver la pantalla en la terminal
```

El test `init_y_volcado_por_i2c` verifica con `embedded_hal_mock::eh1::i2c::Mock` los 25
bytes de inicialización y las 64 transacciones de datos del volcado.

## Hardware y flasheo (Raspberry Pi Pico)

| Señal | Pin Pico |
|---|---|
| OLED SDA / SCL | GP4 / GP5 (I2C0), VCC 3V3 |
| Botón MODO / MÁS / SNOOZE | GP10 / GP11 / GP12 a GND (pull‑up interno) |
| Buzzer activo | GP15 (vía transistor NPN si consume > 12 mA) |

```bash
rustup target add thumbv6m-none-eabi
cargo install elf2uf2-rs        # o probe-rs para depurar con una Debug Probe
# En el crate del firmware (plantilla rp-rs/rp2040-project-template o embassy-rp):
#   let i2c = rp2040_hal::I2C::i2c0(pac.I2C0, sda, scl, 400.kHz(), &mut pac.RESETS, &clocks);
#   let mut oled = Ssd1306::new(i2c, DIRECCION_SSD1306); oled.iniciar()?;
#   loop cada 5 ms: muestrear botones → d.tecla(..); d.tick(5); cada 100 ms: dibujar + volcar
cargo run --release             # con runner = "elf2uf2-rs -d" en .cargo/config.toml (BOOTSEL apretado)
```

La hora puede venir del RTC interno del RP2040 o de un DS3231 externo para no perderla al cortar la luz.

## Limitaciones

- No se probó en una Pico real (sin hardware en este entorno).
- Sin auto‑repetición al mantener MÁS en ajuste (cada pulsación suma 1).
