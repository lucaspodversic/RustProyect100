//! Todas las estrategias deben dar el mismo resultado; además se comprueban
//! los órdenes de magnitud de tiempo de la carga de I/O, que son estables
//! porque dependen del reloj y no de la CPU.

use benchmark_concurrencia::cpu::{self, Estrategia};
use benchmark_concurrencia::io::{self, EstrategiaIo};
use benchmark_concurrencia::medir;
use std::time::Duration;

#[test]
fn cpu_todas_las_estrategias_coinciden() {
    let ts = cpu::tareas(200_000, 37);
    let esperado = cpu::contar(0..200_000);
    assert_eq!(esperado, 17_984);
    for e in Estrategia::TODAS {
        for hilos in [1, 3, 8] {
            assert_eq!(cpu::ejecutar(e, &ts, hilos), esperado, "{e:?} hilos={hilos}");
        }
    }
}

#[test]
fn io_todas_coinciden() {
    let n = 50;
    for e in [
        EstrategiaIo::Secuencial,
        EstrategiaIo::HiloPorPeticion,
        EstrategiaIo::Rayon,
        EstrategiaIo::Tokio,
        EstrategiaIo::TokioLimitado(7),
    ] {
        assert_eq!(io::ejecutar(e, n, 1, 4), n * (n - 1), "{e:?}");
    }
}

#[test]
fn tokio_solapa_miles_de_esperas_con_pocos_hilos() {
    // 5000 esperas de 50 ms en 2 hilos: secuencialmente serían 250 s.
    let (r, t) = medir(|| io::ejecutar(EstrategiaIo::Tokio, 5000, 50, 2));
    assert_eq!(r, 5000 * 4999);
    assert!(t < Duration::from_secs(3), "tardó {t:?}");
}

#[test]
fn rayon_bloquea_con_sleep_y_el_semaforo_limita() {
    // 16 esperas de 30 ms en 2 hilos de rayon → ≥ 8 rondas = 240 ms.
    let (_, t) = medir(|| io::ejecutar(EstrategiaIo::Rayon, 16, 30, 2));
    assert!(t >= Duration::from_millis(230), "rayon {t:?}");
    // Semáforo de 4 con 16 esperas de 30 ms → ≥ 4 rondas = 120 ms.
    let (_, t) = medir(|| io::ejecutar(EstrategiaIo::TokioLimitado(4), 16, 30, 8));
    assert!(t >= Duration::from_millis(115), "semáforo {t:?}");
}
