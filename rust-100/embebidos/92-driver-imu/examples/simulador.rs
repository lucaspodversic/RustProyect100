//! Un MPU‑6050 **simulado** que implementa el lado esclavo del bus: guarda
//! un banco de 128 registros y responde a escrituras y lecturas como el chip
//! real (con auto‑incremento de dirección). Así el driver corre completo en
//! el PC: `cargo run --example simulador`.
use driver_imu::registros::*;
use driver_imu::{Dlpf, Mpu6050, RangoAcel, RangoGiro};
use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::{ErrorKind, ErrorType, I2c, Operation};

struct ChipSimulado {
    regs: [u8; 128],
    puntero: usize,
}

impl ErrorType for ChipSimulado {
    type Error = ErrorKind;
}

impl I2c for ChipSimulado {
    fn transaction(&mut self, dir: u8, ops: &mut [Operation<'_>]) -> Result<(), ErrorKind> {
        if dir != DIR_DEFECTO {
            return Err(ErrorKind::NoAcknowledge(embedded_hal::i2c::NoAcknowledgeSource::Address));
        }
        for op in ops {
            match op {
                // Primer byte = registro; el resto son datos con auto‑incremento.
                Operation::Write(b) => {
                    self.puntero = b[0] as usize;
                    for v in &b[1..] {
                        self.regs[self.puntero] = *v;
                        self.puntero += 1;
                    }
                }
                Operation::Read(b) => {
                    for v in b.iter_mut() {
                        *v = self.regs[self.puntero];
                        self.puntero += 1;
                    }
                }
            }
        }
        Ok(())
    }
}

struct SinEspera;
impl DelayNs for SinEspera {
    fn delay_ns(&mut self, _: u32) {}
}

fn main() {
    let mut chip = ChipSimulado { regs: [0; 128], puntero: 0 };
    chip.regs[WHO_AM_I as usize] = 0x68;
    // Sensor inclinado 30° sobre X, con ±4 g: Y = sin30·8192, Z = cos30·8192.
    let valores: [i16; 7] = [0, 4096, 7094, 0, 40, -20, 15];
    for (i, v) in valores.iter().enumerate() {
        let [h, l] = v.to_be_bytes();
        chip.regs[ACCEL_XOUT_H as usize + 2 * i] = h;
        chip.regs[ACCEL_XOUT_H as usize + 2 * i + 1] = l;
    }
    let mut mpu = Mpu6050::new(chip, DIR_DEFECTO);
    mpu.iniciar(&mut SinEspera, RangoAcel::G4, RangoGiro::Dps500, Dlpf::Hz44, 4).unwrap();
    let bias = mpu.calibrar_giro(&mut SinEspera, 50).unwrap();
    println!("bias giro (cuentas): {bias:?}");
    let d = mpu.leer_crudo().unwrap();
    let a = mpu.acel_mg(&d);
    println!("acel (mg): {a:?}   giro (m°/s): {:?}   temp: {:.2} °C", mpu.giro_mdps(&d), d.temp_centesimas() as f32 / 100.0);
    let roll = (a[1] as f32).atan2(a[2] as f32).to_degrees();
    println!("ángulo de inclinación (roll) estimado: {roll:.1}°");
    let chip = mpu.liberar();
    println!("ACCEL_CONFIG=0x{:02X} GYRO_CONFIG=0x{:02X} PWR_MGMT_1=0x{:02X}", chip.regs[0x1C], chip.regs[0x1B], chip.regs[0x6B]);
}
