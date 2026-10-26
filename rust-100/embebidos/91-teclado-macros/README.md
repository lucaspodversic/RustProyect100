# 91 · Teclado de macros (macropad) para RP2040

Firmware para un macropad de **3×3 teclas** con **2 capas**, **macros** y reportes **USB HID** de arranque. Toda la lógica es `no_std` y se prueba en el PC con una matriz simulada.

```text
pines ─► matriz ─► antirrebote ─► capas / macros ─► reporte HID (8 bytes) ─► USB
```

## Módulos

| Archivo | Contenido |
|---|---|
| `src/matriz.rs` | Escaneo de filas (`OutputPin`, activas en bajo) y columnas (`InputPin` con pull‑up). |
| `src/antirrebote.rs` | Contador por tecla: acepta un cambio tras 5 escaneos estables y emite `Pulsada`/`Soltada`. |
| `src/mapa.rs` | `Accion` (Tecla, CapaMomentanea, CapaAlternar, Macro, Transparente), mapa por defecto y resolución por capas. |
| `src/macros.rs` | `Paso` (Toque, Texto, Esperar) y `Ejecutor` no bloqueante, que inserta un reporte de "soltar" entre caracteres. |
| `src/hid.rs` | Códigos HID, reporte de 8 bytes con 6KRO y *ErrorRollOver*, tabla ASCII→HID (US). |
| `src/teclado.rs` | Integra todo. Las acciones se "congelan" al pulsar, así que una tecla no queda pegada si se suelta Fn antes. |
| `tests/integracion.rs` | Matriz simulada con `Rc<RefCell>` y pruebas de Ctrl+C, Fn, macro de texto y varias teclas a la vez. |

## Mapa por defecto

```text
Capa 0                       Capa 1 (mantener Fn)
Ctrl+C  Ctrl+V  Ctrl+Z       Vol-   Mute   Vol+
Ctrl+S  Macro0  Macro1       F13    ·      ·
Esc     Enter   Fn           ·      ·      (Fn)
```

- Macro0 escribe una firma de correo.
- Macro1 pulsa GUI, espera y escribe `terminal⏎`.

## Probar

```bash
cargo test                 # 10 tests (unitarios + integración, con embedded-hal-mock)
cargo run --example demo   # imprime los reportes de la macro 1
```

## Llevarlo al RP2040 (Raspberry Pi Pico)

1. **Pines**: filas en GP2–GP4 (salidas push‑pull), columnas en GP5–GP7 (entradas con pull‑up). Cada tecla lleva un diodo 1N4148 con el cátodo hacia la fila.
2. **Dependencias**: `rp2040-hal`, `cortex-m-rt`, `usb-device` y `usbd-hid`. Crea un `HIDClass` con `KeyboardReport::desc()` y envía `Reporte::bytes()` en cada tick de 1 ms (desde la interrupción USB o un timer).
3. **Bucle principal**: `if let Some(r) = teclado.tick() { hid.push_raw_input(&r.bytes()) }`.
4. **Compilar y flashear**: `rustup target add thumbv6m-none-eabi`, luego `cargo run --release` usando `elf2uf2-rs -d` o `probe-rs run` como runner en `.cargo/config.toml`.

Las teclas multimedia (Vol±/Mute) usan códigos de la página Keyboard. Linux las acepta, pero en otros sistemas puede hacer falta un reporte *Consumer Control*. Esto se explica más abajo en Limitaciones.

## Limitaciones

- No se compiló para `thumbv6m-none-eabi` porque el entorno no tiene `rustup`. Tampoco se probó en una placa física.
- La tabla ASCII→HID asume la distribución US. Los caracteres no soportados (como `,`, `ñ` o acentos) se omiten.
- Solo usa el reporte de arranque 6KRO (no NKRO) y no tiene reporte *Consumer Control*.
