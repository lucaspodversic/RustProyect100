# 90 · Riego automático por humedad del suelo (Rust `no_std`)

Este controlador maneja **N zonas** de riego. Cada zona tiene un sensor capacitivo y una electroválvula, y todas comparten **una bomba**. Usa `embedded-hal 1.0`, así que funciona en ESP32, RP2040, STM32 y similares.

## Reglas de control

- **Histéresis**: la zona riega por debajo del 30 % y corta al llegar al 45 %. También corta cuando se cumple el tiempo máximo de riego.
- **Remojo** (*soak*): después de regar hay una pausa de 20 min para que el agua penetre en el suelo.
- **Prioridad**: si hay varias zonas pendientes, riega primero la más seca. Solo hay una válvula abierta a la vez.
- **Bloqueos**: no riega si llueve (sensor YL‑83), si el tanque está vacío (flotante) o fuera de la franja horaria (por defecto 19:00→10:00).
- **Falla segura**: si una lectura es imposible (cable cortado o en corto), esa zona se deshabilita. Si falla la lectura de lluvia o del tanque, el sistema asume la condición insegura.
- **Orden de salidas**: al regar se abre primero la válvula y después se enciende la bomba. Al parar se apaga primero la bomba y después se cierra la válvula.
- **Estadísticas**: guarda los litros y los ciclos de riego de cada zona durante el día.

## Módulos

| Archivo | Contenido |
|---|---|
| `src/sensor.rs` | Trait `Adc`, calibración seco/mojado → %, detección de falla y mediana de 5 lecturas. |
| `src/zona.rs` | `ConfigZona`, `EstadoZona` (Ok/Pendiente/Regando/Remojo/FallaSensor) y contadores. |
| `src/controlador.rs` | `Riego::tick(minuto, dt)`, `Franja` (cruza medianoche) y `Bloqueo`. |
| `examples/simular.rs` | Simula 3 días con un modelo de evaporación. |

## Probar

```bash
cargo test                   # 8 tests (fakes + embedded-hal-mock)
cargo run --example simular
```

## Hardware real

| Elemento | Conexión típica (ESP32) |
|---|---|
| Sensores capacitivos | ADC1: GPIO32, GPIO33… (3,3 V) |
| Electroválvulas 12 V | módulo de relés u opto+MOSFET en GPIO25, GPIO26… (diodo flyback) |
| Bomba | relé/MOSFET en GPIO27 |
| Sensor de lluvia YL‑83 (salida D0) | GPIO14 con pull‑up |
| Flotante del tanque | GPIO12 a GND, con pull‑up interno |

Con `esp-hal`, implementa `Adc` sobre `esp_hal::analog::adc::Adc::read_oneshot`. Para flashear: `espflash flash --monitor`. Para obtener la hora usa SNTP (Wi‑Fi) o un RTC DS3231.

## Limitaciones

- No está probado con hardware físico. No calcula evapotranspiración ni usa pronóstico del clima.
- El modelo de suelo del ejemplo es lineal y solo sirve como ilustración.
