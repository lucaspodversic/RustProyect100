//! Driver del sensor de temperatura/humedad **Sensirion SHT3x**.
//!
//! ## Protocolo (resumen de la hoja de datos)
//! 1. El maestro escribe un comando de 16 bits (big‑endian). Usamos
//!    `0x2400` = *single shot, repetibilidad alta, sin clock stretching*.
//! 2. Se espera la conversión (máx. 15,5 ms con repetibilidad alta).
//! 3. Se leen 6 bytes: `T_msb T_lsb CRC_T  RH_msb RH_lsb CRC_RH`.
//! 4. Cada par de bytes viene protegido por un CRC‑8 (polinomio 0x31,
//!    valor inicial 0xFF, sin reflexión, sin XOR final).
//!
//! ## Conversión a unidades físicas
//! - `T [°C]  = -45 + 175 · raw / 65535`
//! - `RH [%]  = 100 · raw / 65535`
//!
//! Trabajamos en **enteros** (centésimas de °C y de %) para no depender de
//! una FPU: muchos microcontroladores (p. ej. el Cortex‑M0+ del RP2040) no la
//! tienen y el punto flotante por software es lento y ocupa flash.

use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;

/// Dirección I2C por defecto (pin ADDR a GND). Con ADDR a VDD es `0x45`.
pub const DIRECCION_POR_DEFECTO: u8 = 0x44;

/// Comando "single shot, alta repetibilidad, sin clock stretching".
pub const CMD_MEDIR_ALTA: [u8; 2] = [0x24, 0x00];
/// Comando de *soft reset*.
pub const CMD_SOFT_RESET: [u8; 2] = [0x30, 0xA2];

/// Tiempo de espera de la conversión en milisegundos (hoja de datos: 15,5 ms).
pub const ESPERA_MEDICION_MS: u32 = 16;

/// Una lectura ya convertida a unidades físicas (en centésimas).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lectura {
    /// Temperatura en centésimas de grado Celsius (2153 = 21,53 °C).
    pub temp_centi_c: i32,
    /// Humedad relativa en centésimas de porcentaje (4512 = 45,12 %).
    pub humedad_centi_pct: u32,
}

/// Errores que puede devolver el driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error<E> {
    /// Falló el bus I2C (NACK, arbitraje, etc.). Se envuelve el error del HAL.
    Bus(E),
    /// El CRC recibido no coincide con el calculado: dato corrupto.
    Crc {
        /// CRC que calculamos nosotros.
        esperado: u8,
        /// CRC que mandó el sensor.
        recibido: u8,
    },
}

/// Calcula el CRC‑8 de Sensirion (poly 0x31, init 0xFF).
///
/// Implementación bit a bit: más lenta que una tabla, pero ocupa ~20 bytes
/// de flash en vez de 256, lo cual es preferible en un MCU chico.
pub fn crc8(datos: &[u8]) -> u8 {
    let mut crc: u8 = 0xFF;
    for &byte in datos {
        crc ^= byte;
        for _ in 0..8 {
            // Si el bit más significativo está en 1, "restamos" (XOR) el
            // polinomio después de desplazar; si no, solo desplazamos.
            crc = if crc & 0x80 != 0 {
                (crc << 1) ^ 0x31
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Convierte el valor crudo de temperatura a centésimas de °C.
///
/// Usamos `i64` en el producto intermedio para que `17500 * 65535` no
/// desborde (cabe en i32, pero así queda explícito y seguro).
pub fn convertir_temperatura(raw: u16) -> i32 {
    (-4500 + (17500i64 * raw as i64) / 65535) as i32
}

/// Convierte el valor crudo de humedad a centésimas de %.
pub fn convertir_humedad(raw: u16) -> u32 {
    ((10000u64 * raw as u64) / 65535) as u32
}

/// Driver del SHT3x. Es *dueño* del bus I2C (patrón típico de embedded-hal);
/// si el bus se comparte con otros dispositivos se puede pasar un
/// `embedded_hal_bus::i2c::RefCellDevice` o similar.
pub struct Sht3x<I2C> {
    i2c: I2C,
    direccion: u8,
}

impl<I2C: I2c> Sht3x<I2C> {
    /// Crea el driver con la dirección indicada.
    pub fn new(i2c: I2C, direccion: u8) -> Self {
        Self { i2c, direccion }
    }

    /// Devuelve el bus (por ejemplo para reutilizarlo o para `done()` en tests).
    pub fn liberar(self) -> I2C {
        self.i2c
    }

    /// Envía un *soft reset* al sensor.
    pub fn reset<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<I2C::Error>> {
        self.i2c
            .write(self.direccion, &CMD_SOFT_RESET)
            .map_err(Error::Bus)?;
        // La hoja de datos pide ≥1,5 ms tras el reset.
        delay.delay_ms(2);
        Ok(())
    }

    /// Realiza una medición *single shot* completa y devuelve la lectura.
    pub fn medir<D: DelayNs>(&mut self, delay: &mut D) -> Result<Lectura, Error<I2C::Error>> {
        // 1) Disparamos la conversión.
        self.i2c
            .write(self.direccion, &CMD_MEDIR_ALTA)
            .map_err(Error::Bus)?;
        // 2) Esperamos a que el sensor termine de convertir.
        delay.delay_ms(ESPERA_MEDICION_MS);
        // 3) Leemos los 6 bytes de resultado.
        let mut buf = [0u8; 6];
        self.i2c.read(self.direccion, &mut buf).map_err(Error::Bus)?;
        // 4) Validamos cada palabra con su CRC.
        let raw_t = verificar_palabra(&buf[0..3])?;
        let raw_h = verificar_palabra(&buf[3..6])?;
        Ok(Lectura {
            temp_centi_c: convertir_temperatura(raw_t),
            humedad_centi_pct: convertir_humedad(raw_h),
        })
    }
}

/// Verifica `[msb, lsb, crc]` y devuelve la palabra de 16 bits.
fn verificar_palabra<E>(tres: &[u8]) -> Result<u16, Error<E>> {
    let esperado = crc8(&tres[0..2]);
    if esperado != tres[2] {
        return Err(Error::Crc {
            esperado,
            recibido: tres[2],
        });
    }
    Ok(u16::from_be_bytes([tres[0], tres[1]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::delay::NoopDelay;
    use embedded_hal_mock::eh1::i2c::{Mock, Transaction};

    #[test]
    fn crc_de_la_hoja_de_datos() {
        // Ejemplo oficial de Sensirion: CRC(0xBEEF) = 0x92.
        assert_eq!(crc8(&[0xBE, 0xEF]), 0x92);
    }

    #[test]
    fn conversiones_en_los_extremos() {
        assert_eq!(convertir_temperatura(0), -4500);
        assert_eq!(convertir_temperatura(0xFFFF), 13000);
        assert_eq!(convertir_humedad(0), 0);
        assert_eq!(convertir_humedad(0xFFFF), 10000);
    }

    /// Arma la respuesta de 6 bytes con CRCs válidos.
    fn respuesta(raw_t: u16, raw_h: u16) -> Vec<u8> {
        let t = raw_t.to_be_bytes();
        let h = raw_h.to_be_bytes();
        vec![t[0], t[1], crc8(&t), h[0], h[1], crc8(&h)]
    }

    #[test]
    fn medicion_completa_con_mock() {
        // raw_t = 0x6666 → ≈ 25 °C ; raw_h = 0x8000 → ≈ 50 %.
        let esperado = [
            Transaction::write(0x44, CMD_MEDIR_ALTA.to_vec()),
            Transaction::read(0x44, respuesta(0x6666, 0x8000)),
        ];
        let mut sensor = Sht3x::new(Mock::new(&esperado), 0x44);
        let lectura = sensor.medir(&mut NoopDelay::new()).unwrap();
        assert_eq!(lectura.temp_centi_c, 2500);
        assert_eq!(lectura.humedad_centi_pct, 5000);
        sensor.liberar().done(); // verifica que no sobren transacciones
    }

    #[test]
    fn crc_corrupto_se_detecta() {
        let mut r = respuesta(0x6666, 0x8000);
        r[2] ^= 0xFF; // rompemos el CRC de la temperatura
        let esperado = [
            Transaction::write(0x44, CMD_MEDIR_ALTA.to_vec()),
            Transaction::read(0x44, r),
        ];
        let mut sensor = Sht3x::new(Mock::new(&esperado), 0x44);
        let err = sensor.medir(&mut NoopDelay::new()).unwrap_err();
        assert!(matches!(err, Error::Crc { .. }));
        sensor.liberar().done();
    }

    #[test]
    fn error_de_bus_se_propaga() {
        use embedded_hal::i2c::ErrorKind;
        let esperado = [Transaction::write(0x44, CMD_MEDIR_ALTA.to_vec())
            .with_error(ErrorKind::Other)];
        let mut sensor = Sht3x::new(Mock::new(&esperado), 0x44);
        assert!(matches!(
            sensor.medir(&mut NoopDelay::new()),
            Err(Error::Bus(_))
        ));
        sensor.liberar().done();
    }

    #[test]
    fn reset_envia_comando() {
        let esperado = [Transaction::write(0x44, CMD_SOFT_RESET.to_vec())];
        let mut sensor = Sht3x::new(Mock::new(&esperado), 0x44);
        sensor.reset(&mut NoopDelay::new()).unwrap();
        sensor.liberar().done();
    }
}
