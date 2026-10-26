//! Simulación en la PC: un SHT3x "virtual" que genera una curva diaria de
//! temperatura y un Wi‑Fi que imprime por consola. Ejecutar con:
//!
//! ```text
//! cargo run --example simulacion_host
//! ```
use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::{ErrorKind, ErrorType, I2c, Operation};
use estacion_meteorologica::estacion::{sensor_por_defecto, Config, Estacion, Evento};
use estacion_meteorologica::red::{ErrorRed, Transporte};
use estacion_meteorologica::sht3x::crc8;

/// Sensor virtual: cada medición avanza "una hora" en una senoide simple.
struct SensorVirtual {
    hora: u32,
}

impl ErrorType for SensorVirtual {
    type Error = ErrorKind;
}

impl I2c for SensorVirtual {
    fn transaction(&mut self, _addr: u8, ops: &mut [Operation<'_>]) -> Result<(), Self::Error> {
        for op in ops {
            if let Operation::Read(buf) = op {
                // Temperatura entre ~15 y ~27 °C con pico a las 15 h (aprox. lineal por tramos).
                let h = (self.hora % 24) as i32;
                let dist = (h - 15).abs().min(24 - (h - 15).abs());
                let temp_c = 27.0 - dist as f32;
                let hum = 40.0 + dist as f32 * 2.5;
                let raw_t = (((temp_c + 45.0) / 175.0) * 65535.0) as u16;
                let raw_h = ((hum / 100.0) * 65535.0) as u16;
                let t = raw_t.to_be_bytes();
                let hh = raw_h.to_be_bytes();
                buf.copy_from_slice(&[t[0], t[1], crc8(&t), hh[0], hh[1], crc8(&hh)]);
                self.hora += 1;
            }
        }
        Ok(())
    }
}

struct WifiConsola;
impl Transporte for WifiConsola {
    fn conectar(&mut self) -> Result<(), ErrorRed> {
        Ok(())
    }
    fn conectado(&self) -> bool {
        true
    }
    fn enviar(&mut self, p: &[u8]) -> Result<(), ErrorRed> {
        println!("POST /api/lecturas  {}", String::from_utf8_lossy(p));
        Ok(())
    }
}

struct SinEspera;
impl DelayNs for SinEspera {
    fn delay_ns(&mut self, _: u32) {}
}

fn main() {
    let cfg = Config { id_estacion: 1, muestras_por_envio: 6, reintentos: Default::default() };
    let mut est = Estacion::new(sensor_por_defecto(SensorVirtual { hora: 0 }), WifiConsola, cfg);
    for _ in 0..24 {
        if let Evento::Medido(l) = est.tick(&mut SinEspera) {
            println!("  medido: {} c°C, {} c%", l.temp_centi_c, l.humedad_centi_pct);
        }
    }
}
