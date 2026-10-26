//! Vuelo simulado con perturbaciones; imprime la actitud cada 100 ms.
//! `cargo run --example vuelo`
use firmware_dron::control::{Controlador, Ganancias, Radio};
use firmware_dron::seguridad::Limites;
use firmware_dron::simulacion::{Dron, Fisica};

fn main() {
    let dt = 0.001;
    let mut c = Controlador::new(Ganancias::default(), Limites::default());
    let mut d = Dron::new(Fisica::default(), 0.0, 0.0);
    let mut radio = Radio { enlace: true, ..Default::default() };
    for _ in 0..20 {
        let imu = d.imu();
        c.ciclo(&imu, &radio, dt);
    }
    c.armar(&radio).expect("armado");
    println!("  t(s)  fase              roll(°) pitch(°) yaw/s  motores");
    for ms in 0..6000u32 {
        let fase = match ms {
            0..=999 => { radio.acelerador = 0.5; "despegue" }
            1000..=1999 => { if ms == 1000 { d.angulo[0] = 25.0; } "ráfaga roll 25°" }
            2000..=2999 => { radio.pitch = 0.33; "avance pitch 10°" }
            3000..=3999 => { radio.pitch = 0.0; radio.yaw = 0.5; "giro yaw 90°/s" }
            _ => { radio.yaw = 0.0; radio.enlace = false; "¡radio perdida!" }
        };
        let imu = d.imu();
        let t = c.ciclo(&imu, &radio, dt);
        d.paso(t.motores, dt);
        if ms % 250 == 0 {
            println!(
                "{:6.2}  {:<17} {:7.2} {:8.2} {:6.1}  {:.2?}  {:?}",
                ms as f32 / 1000.0, fase, d.angulo[0], d.angulo[1], d.velocidad[2], t.motores, t.estado
            );
        }
    }
}
