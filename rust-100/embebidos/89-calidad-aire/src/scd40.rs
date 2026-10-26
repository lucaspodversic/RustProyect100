//! Driver del sensor de CO₂ **Sensirion SCD40/SCD41**.
//!
//! Comandos usados (16 bits, big‑endian):
//! - `0x21B1` start_periodic_measurement (una medición cada 5 s).
//! - `0xE4B8` get_data_ready_status → palabra; listo si `(w & 0x07FF) != 0`.
//! - `0xEC05` read_measurement → 3 palabras con CRC: CO₂ (ppm),
//!   T = −45 + 175·raw/65535, HR = 100·raw/65535.
//! - `0x3F86` stop_periodic_measurement.
//!
//! El CRC‑8 es el mismo de toda la familia Sensirion (poly 0x31, init 0xFF).

use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;

/// Dirección I2C fija del SCD4x.
pub const DIRECCION: u8 = 0x62;

/// Medición del SCD40.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MedicionCo2 {
    /// CO₂ en ppm.
    pub co2_ppm: u16,
    /// Temperatura en centésimas de °C.
    pub temp_centi: i32,
    /// Humedad en centésimas de %.
    pub hum_centi: u32,
}

/// Errores del driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error<E> {
    /// Error del bus.
    Bus(E),
    /// CRC inválido.
    Crc,
    /// Todavía no hay medición nueva.
    NoListo,
}

/// CRC‑8 Sensirion.
pub fn crc8(d: &[u8]) -> u8 {
    let mut crc = 0xFFu8;
    for &b in d {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0x31 } else { crc << 1 };
        }
    }
    crc
}

/// Driver SCD40.
pub struct Scd40<I2C> {
    i2c: I2C,
}

impl<I2C: I2c> Scd40<I2C> {
    /// Crea el driver.
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }
    /// Devuelve el bus.
    pub fn liberar(self) -> I2C {
        self.i2c
    }

    fn comando(&mut self, cmd: u16) -> Result<(), Error<I2C::Error>> {
        self.i2c.write(DIRECCION, &cmd.to_be_bytes()).map_err(Error::Bus)
    }

    /// Lee `N` palabras tras enviar `cmd` (con la pausa de 1 ms que pide la hoja de datos).
    fn leer_palabras<const N: usize, D: DelayNs>(
        &mut self,
        cmd: u16,
        delay: &mut D,
    ) -> Result<[u16; N], Error<I2C::Error>> {
        self.comando(cmd)?;
        delay.delay_ms(1);
        let mut buf = [0u8; 9];
        let buf = &mut buf[..N * 3];
        self.i2c.read(DIRECCION, buf).map_err(Error::Bus)?;
        let mut out = [0u16; N];
        for (i, tres) in buf.chunks(3).enumerate() {
            if crc8(&tres[..2]) != tres[2] {
                return Err(Error::Crc);
            }
            out[i] = u16::from_be_bytes([tres[0], tres[1]]);
        }
        Ok(out)
    }

    /// Inicia la medición periódica.
    pub fn iniciar(&mut self) -> Result<(), Error<I2C::Error>> {
        self.comando(0x21B1)
    }

    /// Detiene la medición periódica (necesita 500 ms antes de otro comando).
    pub fn detener<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<I2C::Error>> {
        self.comando(0x3F86)?;
        delay.delay_ms(500);
        Ok(())
    }

    /// Lee una medición si hay una lista; si no, `Error::NoListo`.
    pub fn leer<D: DelayNs>(&mut self, delay: &mut D) -> Result<MedicionCo2, Error<I2C::Error>> {
        let [estado] = self.leer_palabras::<1, D>(0xE4B8, delay)?;
        if estado & 0x07FF == 0 {
            return Err(Error::NoListo);
        }
        let [co2, t, h] = self.leer_palabras::<3, D>(0xEC05, delay)?;
        Ok(MedicionCo2 {
            co2_ppm: co2,
            temp_centi: (-4500 + 17500i64 * t as i64 / 65535) as i32,
            hum_centi: (10000u64 * h as u64 / 65535) as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::delay::NoopDelay;
    use embedded_hal_mock::eh1::i2c::{Mock, Transaction};

    fn palabras(ws: &[u16]) -> Vec<u8> {
        ws.iter()
            .flat_map(|w| {
                let b = w.to_be_bytes();
                [b[0], b[1], crc8(&b)]
            })
            .collect()
    }

    #[test]
    fn lectura_completa() {
        let tx = [
            Transaction::write(DIRECCION, vec![0x21, 0xB1]),
            Transaction::write(DIRECCION, vec![0xE4, 0xB8]),
            Transaction::read(DIRECCION, palabras(&[0x8006])),
            Transaction::write(DIRECCION, vec![0xEC, 0x05]),
            Transaction::read(DIRECCION, palabras(&[650, 0x6666, 0x8000])),
        ];
        let mut s = Scd40::new(Mock::new(&tx));
        s.iniciar().unwrap();
        let m = s.leer(&mut NoopDelay::new()).unwrap();
        assert_eq!(m, MedicionCo2 { co2_ppm: 650, temp_centi: 2500, hum_centi: 5000 });
        s.liberar().done();
    }

    #[test]
    fn no_listo_y_crc() {
        let mut malo = palabras(&[0x8006]);
        malo[2] ^= 1;
        let tx = [
            Transaction::write(DIRECCION, vec![0xE4, 0xB8]),
            Transaction::read(DIRECCION, palabras(&[0x8000])),
            Transaction::write(DIRECCION, vec![0xE4, 0xB8]),
            Transaction::read(DIRECCION, malo),
        ];
        let mut s = Scd40::new(Mock::new(&tx));
        assert_eq!(s.leer(&mut NoopDelay::new()), Err(Error::NoListo));
        assert_eq!(s.leer(&mut NoopDelay::new()), Err(Error::Crc));
        s.liberar().done();
    }
}
