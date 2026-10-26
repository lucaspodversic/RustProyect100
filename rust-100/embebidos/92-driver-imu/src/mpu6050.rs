//! Driver del MPU‑6050.

use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;

use crate::registros::*;

/// Errores del driver.
#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> {
    /// Error del bus I2C (NACK, arbitraje, etc.).
    Bus(E),
    /// WHO_AM_I devolvió un valor inesperado (¿otro chip? ¿clon?).
    IdIncorrecto(u8),
}

impl<E> From<E> for Error<E> {
    fn from(e: E) -> Self {
        Error::Bus(e)
    }
}

/// Lectura cruda de los 7 canales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Datos {
    /// Aceleración X, Y, Z en cuentas.
    pub acel: [i16; 3],
    /// Temperatura en cuentas.
    pub temp: i16,
    /// Velocidad angular X, Y, Z en cuentas.
    pub giro: [i16; 3],
}

impl Datos {
    /// Decodifica los 14 bytes big‑endian leídos desde ACCEL_XOUT_H.
    pub fn desde_bytes(b: &[u8; 14]) -> Self {
        let p = |i: usize| i16::from_be_bytes([b[i], b[i + 1]]);
        Self { acel: [p(0), p(2), p(4)], temp: p(6), giro: [p(8), p(10), p(12)] }
    }

    /// Temperatura en centésimas de °C. Fórmula de la hoja de datos:
    /// `T = raw / 340 + 36,53` → en centésimas: `raw·100/340 + 3653`.
    pub fn temp_centesimas(&self) -> i32 {
        self.temp as i32 * 100 / 340 + 3653
    }
}

/// Driver. `I` es el bus I2C; el driver se queda con él (usa
/// `embedded-hal-bus` si necesitas compartirlo con otros dispositivos).
pub struct Mpu6050<I> {
    i2c: I,
    dir: u8,
    rango_acel: RangoAcel,
    rango_giro: RangoGiro,
    /// Bias del giroscopio restado en cada lectura (cuentas).
    bias_giro: [i16; 3],
}

impl<I: I2c> Mpu6050<I> {
    /// Crea el driver sin tocar el bus.
    pub fn new(i2c: I, dir: u8) -> Self {
        Self { i2c, dir, rango_acel: RangoAcel::G2, rango_giro: RangoGiro::Dps250, bias_giro: [0; 3] }
    }

    /// Devuelve el bus (útil para tests con mocks).
    pub fn liberar(self) -> I {
        self.i2c
    }

    fn escribir(&mut self, reg: u8, val: u8) -> Result<(), Error<I::Error>> {
        Ok(self.i2c.write(self.dir, &[reg, val])?)
    }

    fn leer(&mut self, reg: u8, buf: &mut [u8]) -> Result<(), Error<I::Error>> {
        // write_read genera START + dir(W) + reg + RESTART + dir(R) + datos,
        // sin STOP intermedio, como exige el chip.
        Ok(self.i2c.write_read(self.dir, &[reg], buf)?)
    }

    /// Lee WHO_AM_I.
    pub fn id(&mut self) -> Result<u8, Error<I::Error>> {
        let mut b = [0];
        self.leer(WHO_AM_I, &mut b)?;
        Ok(b[0])
    }

    /// Inicializa el chip:
    /// 1. verifica WHO_AM_I;
    /// 2. hace un reset y espera 100 ms;
    /// 3. lo despierta (al arrancar está en *sleep*) con reloj PLL;
    /// 4. configura DLPF, divisor de muestreo y rangos.
    pub fn iniciar<D: DelayNs>(
        &mut self,
        delay: &mut D,
        acel: RangoAcel,
        giro: RangoGiro,
        dlpf: Dlpf,
        divisor: u8,
    ) -> Result<(), Error<I::Error>> {
        let id = self.id()?;
        if id != ID_ESPERADO {
            return Err(Error::IdIncorrecto(id));
        }
        self.escribir(PWR_MGMT_1, RESET)?;
        delay.delay_ms(100);
        self.escribir(PWR_MGMT_1, RELOJ_PLL_GX)?;
        self.escribir(CONFIG, dlpf as u8)?;
        self.escribir(SMPLRT_DIV, divisor)?;
        self.escribir(ACCEL_CONFIG, acel.registro())?;
        self.escribir(GYRO_CONFIG, giro.registro())?;
        self.rango_acel = acel;
        self.rango_giro = giro;
        Ok(())
    }

    /// Lectura en ráfaga de los 14 bytes, con el bias del giro restado.
    pub fn leer_crudo(&mut self) -> Result<Datos, Error<I::Error>> {
        let mut b = [0u8; 14];
        self.leer(ACCEL_XOUT_H, &mut b)?;
        let mut d = Datos::desde_bytes(&b);
        for (g, bias) in d.giro.iter_mut().zip(self.bias_giro) {
            *g = g.saturating_sub(bias);
        }
        Ok(d)
    }

    /// Aceleración en mili‑g.
    pub fn acel_mg(&self, d: &Datos) -> [i32; 3] {
        d.acel.map(|v| v as i32 * 1000 / self.rango_acel.lsb_por_g())
    }

    /// Velocidad angular en **mili‑°/s** (enteros, sin FPU).
    pub fn giro_mdps(&self, d: &Datos) -> [i32; 3] {
        // cuentas / (LSB/dps) * 1000 = cuentas * 10_000 / (LSB/dps * 10)
        d.giro.map(|v| v as i32 * 10_000 / self.rango_giro.lsb_por_dps_x10())
    }

    /// Calibra el bias del giroscopio promediando `n` muestras con el sensor
    /// **quieto**. En reposo el giro ideal es 0 y lo que se mida es offset.
    pub fn calibrar_giro<D: DelayNs>(&mut self, delay: &mut D, n: u16) -> Result<[i16; 3], Error<I::Error>> {
        self.bias_giro = [0; 3];
        let mut suma = [0i32; 3];
        for _ in 0..n {
            let d = self.leer_crudo()?;
            for (s, g) in suma.iter_mut().zip(d.giro) {
                *s += g as i32;
            }
            delay.delay_ms(2);
        }
        let n = n.max(1) as i32;
        self.bias_giro = suma.map(|s| (s / n) as i16);
        Ok(self.bias_giro)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal::i2c::ErrorKind;
    use embedded_hal_mock::eh1::delay::{CheckedDelay, NoopDelay, Transaction as D};
    use embedded_hal_mock::eh1::i2c::{Mock, Transaction as T};

    const A: u8 = DIR_DEFECTO;

    /// Construye los 14 bytes de una lectura.
    fn bytes(acel: [i16; 3], temp: i16, giro: [i16; 3]) -> Vec<u8> {
        acel.iter()
            .chain(core::iter::once(&temp))
            .chain(giro.iter())
            .flat_map(|v| v.to_be_bytes())
            .collect()
    }

    #[test]
    fn inicializacion_byte_a_byte() {
        let esperadas = [
            T::write_read(A, vec![WHO_AM_I], vec![0x68]),
            T::write(A, vec![PWR_MGMT_1, 0x80]),
            T::write(A, vec![PWR_MGMT_1, 0x01]),
            T::write(A, vec![CONFIG, 3]),
            T::write(A, vec![SMPLRT_DIV, 4]),
            T::write(A, vec![ACCEL_CONFIG, 0x10]),
            T::write(A, vec![GYRO_CONFIG, 0x08]),
        ];
        let mut delay = CheckedDelay::new(&[D::delay_ms(100)]);
        let mut mpu = Mpu6050::new(Mock::new(&esperadas), A);
        mpu.iniciar(&mut delay, RangoAcel::G8, RangoGiro::Dps500, Dlpf::Hz44, 4).unwrap();
        mpu.liberar().done();
        delay.done();
    }

    #[test]
    fn id_incorrecto() {
        let mut mpu = Mpu6050::new(Mock::new(&[T::write_read(A, vec![WHO_AM_I], vec![0x70])]), A);
        let r = mpu.iniciar(&mut NoopDelay::new(), RangoAcel::G2, RangoGiro::Dps250, Dlpf::Hz260, 0);
        assert_eq!(r, Err(Error::IdIncorrecto(0x70)));
        mpu.liberar().done();
    }

    #[test]
    fn error_de_bus_se_propaga() {
        let t = T::write_read(A, vec![WHO_AM_I], vec![0]).with_error(ErrorKind::Other);
        let mut mpu = Mpu6050::new(Mock::new(&[t]), A);
        assert_eq!(mpu.id(), Err(Error::Bus(ErrorKind::Other)));
        mpu.liberar().done();
    }

    #[test]
    fn lectura_y_conversiones() {
        // Plano y quieto con ±2 g: Z = +1 g = 16384; temp raw −1700 → 31,53 °C;
        // giro X = 131 cuentas = 1 °/s.
        let datos = bytes([0, -8192, 16384], -1700, [131, -262, 0]);
        let mut mpu = Mpu6050::new(Mock::new(&[T::write_read(A, vec![ACCEL_XOUT_H], datos)]), A);
        let d = mpu.leer_crudo().unwrap();
        assert_eq!(mpu.acel_mg(&d), [0, -500, 1000]);
        assert_eq!(mpu.giro_mdps(&d), [1000, -2000, 0]);
        assert_eq!(d.temp_centesimas(), 3153);
        mpu.liberar().done();
    }

    #[test]
    fn calibracion_resta_bias() {
        let mut tr: Vec<T> = (0..4)
            .map(|i| T::write_read(A, vec![ACCEL_XOUT_H], bytes([0; 3], 0, [10 + i, -5, 3])))
            .collect();
        tr.push(T::write_read(A, vec![ACCEL_XOUT_H], bytes([0; 3], 0, [11, -5, 3])));
        let mut mpu = Mpu6050::new(Mock::new(&tr), A);
        let bias = mpu.calibrar_giro(&mut NoopDelay::new(), 4).unwrap();
        assert_eq!(bias, [11, -5, 3]); // (10+11+12+13)/4 = 11
        assert_eq!(mpu.leer_crudo().unwrap().giro, [0, 0, 0]);
        mpu.liberar().done();
    }

    #[test]
    fn escalas() {
        assert_eq!(RangoAcel::G16.lsb_por_g(), 2048);
        assert_eq!(RangoAcel::G4.registro(), 0x08);
        assert_eq!(RangoGiro::Dps2000.registro(), 0x18);
    }
}
