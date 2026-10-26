//! Simula el despertador en la terminal: muestra la OLED como ASCII,
//! adelanta el reloj hasta la alarma y pulsa SNOOZE.
//! `cargo run --example simular`
use core::convert::Infallible;
use embedded_hal::digital::{ErrorType, OutputPin};
use reloj_despertador::botones::EventoBoton;
use reloj_despertador::despertador::{Despertador, Tecla};
use reloj_despertador::hora::Hora;
use reloj_despertador::interfaz::dibujar;
use reloj_despertador::pantalla::{Framebuffer, H, W};

struct BuzzerConsola;
impl ErrorType for BuzzerConsola {
    type Error = Infallible;
}
impl OutputPin for BuzzerConsola {
    fn set_high(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
    fn set_low(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}

fn mostrar(fb: &Framebuffer, titulo: &str) {
    println!("── {titulo} ──");
    for y in (0..H).step_by(2) {
        let fila: String = (0..W)
            .map(|x| match (fb.get(x, y), fb.get(x, y + 1)) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                _ => ' ',
            })
            .collect();
        println!("{fila}");
    }
}

fn main() {
    let mut d = Despertador::new(Hora::hms(6, 59, 55), Hora::hms(7, 0, 0), BuzzerConsola);
    let mut fb = Framebuffer::default();
    dibujar(&mut fb, &d, true);
    mostrar(&fb, "06:59:55");
    for _ in 0..5 {
        d.tick(1000);
    }
    dibujar(&mut fb, &d, true);
    mostrar(&fb, &format!("estado {:?}", d.estado()));
    d.tecla(Tecla::Snooze, EventoBoton::Corto);
    dibujar(&mut fb, &d, true);
    mostrar(&fb, &format!("estado {:?}", d.estado()));
}
