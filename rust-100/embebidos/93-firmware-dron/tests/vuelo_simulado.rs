//! Pruebas de lazo cerrado: controlador + simulador físico.

use firmware_dron::control::{Controlador, Ganancias, Radio};
use firmware_dron::seguridad::{Estado, Limites};
use firmware_dron::simulacion::{Dron, Fisica};

const DT: f32 = 0.001;

/// Ejecuta `segundos` de vuelo y devuelve el máximo |roll| y |pitch| reales
/// del último segundo.
fn volar(c: &mut Controlador, d: &mut Dron, radio: &Radio, segundos: f32) -> (f32, f32) {
    let n = (segundos / DT) as usize;
    let (mut mr, mut mp) = (0.0f32, 0.0f32);
    for i in 0..n {
        let imu = d.imu();
        let t = c.ciclo(&imu, radio, DT);
        d.paso(t.motores, DT);
        if i + 1000 >= n {
            mr = mr.max(d.angulo[0].abs());
            mp = mp.max(d.angulo[1].abs());
        }
    }
    (mr, mp)
}

fn armado() -> (Controlador, Dron) {
    let mut c = Controlador::new(Ganancias::default(), Limites::default());
    let mut d = Dron::new(Fisica::default(), 0.0, 0.0);
    // Unas lecturas en el suelo para inicializar el filtro y luego armar.
    let suelo = Radio { enlace: true, ..Default::default() };
    volar(&mut c, &mut d, &suelo, 0.05);
    c.armar(&suelo).unwrap();
    (c, d)
}

#[test]
fn se_estabiliza_tras_una_perturbacion() {
    let (mut c, mut d) = armado();
    let radio = Radio { acelerador: 0.5, enlace: true, ..Default::default() };
    volar(&mut c, &mut d, &radio, 1.0);
    // Ráfaga de viento: inclina el dron 20° de golpe con 100 °/s.
    d.angulo[0] = 20.0;
    d.velocidad[0] = 100.0;
    d.angulo[1] = -10.0;
    let (r, p) = volar(&mut c, &mut d, &radio, 3.0);
    assert!(r < 1.5 && p < 1.5, "roll {r:.2}° pitch {p:.2}°");
}

#[test]
fn sigue_el_setpoint_de_angulo() {
    let (mut c, mut d) = armado();
    // Palanca de roll a la mitad → 15°.
    let radio = Radio { acelerador: 0.5, roll: 0.5, enlace: true, ..Default::default() };
    volar(&mut c, &mut d, &radio, 3.0);
    assert!((d.angulo[0] - 15.0).abs() < 2.0, "roll real {:.2}", d.angulo[0]);
    // Yaw: palanca a 0,25 → 45 °/s.
    let radio = Radio { acelerador: 0.5, yaw: 0.25, enlace: true, ..Default::default() };
    volar(&mut c, &mut d, &radio, 3.0);
    assert!((d.velocidad[2] - 45.0).abs() < 5.0, "yaw rate {:.2}", d.velocidad[2]);
}

#[test]
fn failsafe_nivela_y_desarma() {
    let (mut c, mut d) = armado();
    let radio = Radio { acelerador: 0.6, pitch: 0.6, enlace: true, ..Default::default() };
    volar(&mut c, &mut d, &radio, 1.0);
    assert!(d.angulo[1] > 10.0);
    let sin_radio = Radio { enlace: false, ..radio };
    let (_, p) = volar(&mut c, &mut d, &sin_radio, 2.0);
    assert!(matches!(c.seguridad.estado, Estado::Failsafe { .. }));
    assert!(p < 2.0, "en failsafe se nivela: pitch {p:.2}");
    volar(&mut c, &mut d, &sin_radio, 4.0);
    assert_eq!(c.seguridad.estado, Estado::Desarmado);
    let t = c.ciclo(&d.imu(), &sin_radio, DT);
    assert_eq!(t.motores, [0.0; 4]);
}

#[test]
fn no_arma_inclinado() {
    let mut c = Controlador::new(Ganancias::default(), Limites::default());
    let mut d = Dron::new(Fisica::default(), 35.0, 0.0);
    let suelo = Radio { enlace: true, ..Default::default() };
    for _ in 0..50 {
        let imu = d.imu();
        c.ciclo(&imu, &suelo, DT);
    }
    assert!(c.armar(&suelo).is_err());
}
