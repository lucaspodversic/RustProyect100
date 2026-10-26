//! Simulación de 3 noches con un modelo de suelo muy simple: el suelo se
//! seca 1 %/hora (evaporación) y sube 2 %/min mientras se riega.
//! `cargo run --example simular`
use core::convert::Infallible;
use embedded_hal::digital::{ErrorType, InputPin, OutputPin};
use riego_automatico::controlador::{HwZona, Riego};
use riego_automatico::sensor::Adc;
use riego_automatico::zona::{ConfigZona, Zona};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone)]
struct Suelo(Rc<Cell<f32>>); // humedad %
impl Adc for Suelo {
    fn leer(&mut self) -> Result<u16, ()> {
        Ok((3000.0 - self.0.get() * 17.0) as u16)
    }
}
#[derive(Clone, Default)]
struct Pin(Rc<Cell<bool>>);
impl ErrorType for Pin {
    type Error = Infallible;
}
impl OutputPin for Pin {
    fn set_high(&mut self) -> Result<(), Infallible> {
        self.0.set(true);
        Ok(())
    }
    fn set_low(&mut self) -> Result<(), Infallible> {
        self.0.set(false);
        Ok(())
    }
}
impl InputPin for Pin {
    fn is_high(&mut self) -> Result<bool, Infallible> {
        Ok(true)
    }
    fn is_low(&mut self) -> Result<bool, Infallible> {
        Ok(false)
    }
}

fn main() {
    let suelos = [Rc::new(Cell::new(40.0f32)), Rc::new(Cell::new(30.0f32))];
    let valvulas = [Pin::default(), Pin::default()];
    let hw = [0, 1].map(|i| HwZona { adc: Suelo(suelos[i].clone()), valvula: valvulas[i].clone() });
    let cfg = ConfigZona::default();
    let mut r = Riego::new([Zona::new(cfg); 2], hw, Pin::default(), Pin::default(), Pin::default());
    let mut previo = None;
    for minuto_total in 0..3 * 1440u32 {
        let minuto = (minuto_total % 1440) as u16;
        if minuto == 0 && minuto_total > 0 {
            for (i, z) in r.zonas.iter().enumerate() {
                println!("  fin del día: zona {i} usó {} L en {} ciclos", z.ml_hoy / 1000, z.ciclos_hoy);
            }
            r.nuevo_dia();
        }
        for (i, s) in suelos.iter().enumerate() {
            let mut h = s.get() - 1.0 / 60.0;
            if valvulas[i].0.get() {
                h += 2.0;
            }
            s.set(h.clamp(0.0, 100.0));
        }
        r.tick(minuto, 60_000);
        if r.activa() != previo {
            println!(
                "día {} {:02}:{:02}  zona activa: {:?}  humedades: {:.0}% / {:.0}%",
                minuto_total / 1440 + 1, minuto / 60, minuto % 60, r.activa(),
                suelos[0].get(), suelos[1].get()
            );
            previo = r.activa();
        }
    }
}
