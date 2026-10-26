//! Simulación en host: un "día" de contaminación que sube y baja, mostrando
//! el LCD de 16×2 en la terminal. `cargo run --example simular`
use core::convert::Infallible;
use calidad_aire::monitor::{lineas_lcd, Monitor};
use calidad_aire::pms5003::{construir_trama, leer_trama, Parser};
use embedded_hal::digital::{ErrorType, OutputPin};

struct Led;
impl ErrorType for Led {
    type Error = Infallible;
}
impl OutputPin for Led {
    fn set_high(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
    fn set_low(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}

fn main() {
    // Generamos el flujo de bytes "de la UART" con picos de humo.
    let mut flujo = Vec::new();
    for i in 0..40u16 {
        let pm25 = if (15..25).contains(&i) { 80 + i } else { 8 + i % 5 };
        flujo.extend_from_slice(&construir_trama(pm25 / 2, pm25, pm25 + 10));
    }
    let mut uart: &[u8] = &flujo;
    let mut parser = Parser::default();
    let mut m = Monitor::new(Led);
    for i in 0..40 {
        m.co2(500 + i * 40);
        let t = leer_trama(&mut uart, &mut parser).unwrap();
        let e = m.procesar(&t);
        let (l1, l2) = lineas_lcd(&e);
        println!(
            "t={:>2}s ┃{}┃ ┃{}┃",
            i,
            String::from_utf8_lossy(&l1),
            String::from_utf8_lossy(&l2)
        );
    }
}
