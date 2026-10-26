//! Mapa de registros del MPU‑6050 (hoja "Register Map and Descriptions" rev 4.2).

/// Dirección I2C con AD0 = 0.
pub const DIR_DEFECTO: u8 = 0x68;
/// Dirección I2C con AD0 = 1.
pub const DIR_ALTERNATIVA: u8 = 0x69;

/// Divisor de frecuencia de muestreo: `Fs = Fgyro / (1 + SMPLRT_DIV)`.
pub const SMPLRT_DIV: u8 = 0x19;
/// Configuración general (bits 2..0 = DLPF_CFG).
pub const CONFIG: u8 = 0x1A;
/// Configuración del giroscopio (bits 4..3 = FS_SEL).
pub const GYRO_CONFIG: u8 = 0x1B;
/// Configuración del acelerómetro (bits 4..3 = AFS_SEL).
pub const ACCEL_CONFIG: u8 = 0x1C;
/// Primer registro de datos: ACCEL_XOUT_H. Le siguen en orden
/// ACCEL X/Y/Z, TEMP y GYRO X/Y/Z (14 bytes en big‑endian).
pub const ACCEL_XOUT_H: u8 = 0x3B;
/// Gestión de energía 1 (bit 7 = reset, bit 6 = sleep, bits 2..0 = reloj).
pub const PWR_MGMT_1: u8 = 0x6B;
/// Identidad: debe leer 0x68.
pub const WHO_AM_I: u8 = 0x75;
/// Valor esperado de WHO_AM_I.
pub const ID_ESPERADO: u8 = 0x68;

/// Bit de reset del dispositivo en PWR_MGMT_1.
pub const RESET: u8 = 1 << 7;
/// Fuente de reloj: PLL con referencia en el giroscopio X (más estable que
/// el oscilador interno de 8 MHz, como recomienda la hoja de datos).
pub const RELOJ_PLL_GX: u8 = 0x01;

/// Rango del acelerómetro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RangoAcel {
    /// ±2 g → 16384 LSB/g.
    G2 = 0,
    /// ±4 g → 8192 LSB/g.
    G4 = 1,
    /// ±8 g → 4096 LSB/g.
    G8 = 2,
    /// ±16 g → 2048 LSB/g.
    G16 = 3,
}

impl RangoAcel {
    /// Cuentas por g.
    pub const fn lsb_por_g(self) -> i32 {
        16384 >> (self as u8)
    }
    /// Valor del registro ACCEL_CONFIG.
    pub const fn registro(self) -> u8 {
        (self as u8) << 3
    }
}

/// Rango del giroscopio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RangoGiro {
    /// ±250 °/s → 131 LSB/(°/s).
    Dps250 = 0,
    /// ±500 °/s → 65,5.
    Dps500 = 1,
    /// ±1000 °/s → 32,8.
    Dps1000 = 2,
    /// ±2000 °/s → 16,4.
    Dps2000 = 3,
}

impl RangoGiro {
    /// Cuentas por (°/s) multiplicadas por 10 (para trabajar con enteros).
    pub const fn lsb_por_dps_x10(self) -> i32 {
        match self {
            RangoGiro::Dps250 => 1310,
            RangoGiro::Dps500 => 655,
            RangoGiro::Dps1000 => 328,
            RangoGiro::Dps2000 => 164,
        }
    }
    /// Valor del registro GYRO_CONFIG.
    pub const fn registro(self) -> u8 {
        (self as u8) << 3
    }
}

/// Filtro paso bajo digital (ancho de banda aproximado del acelerómetro).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Dlpf {
    /// 260 Hz (sin filtrar; giroscopio a 8 kHz).
    Hz260 = 0,
    /// 184 Hz.
    Hz184 = 1,
    /// 94 Hz.
    Hz94 = 2,
    /// 44 Hz (buena opción para drones).
    Hz44 = 3,
    /// 21 Hz.
    Hz21 = 4,
    /// 10 Hz.
    Hz10 = 5,
    /// 5 Hz.
    Hz5 = 6,
}
