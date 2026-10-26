use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::path::PathBuf;
use topologia_pcie::modelo::*;
use topologia_pcie::sysfs::*;
use topologia_pcie::topologia::*;
use topologia_pcie::ui::{dibujar, App};

/// Directorio temporal único que se borra al salir del test.
struct Tmp(PathBuf);
impl Tmp {
    fn new(n: &str) -> Self {
        let p = std::env::temp_dir().join(format!("pcie-test-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn demo(n: &str) -> (Tmp, Topologia) {
    let t = Tmp::new(n);
    sysfs_demo(&t.0).unwrap();
    let topo = Topologia::new(leer_dispositivos(&t.0).unwrap());
    (t, topo)
}

fn b(s: &str) -> Bdf {
    Bdf::parsear(s).unwrap()
}

#[test]
fn bdf_y_clases() {
    assert!(Bdf::parsear("0000:3B:00.1").is_some());
    assert_eq!(Bdf::parsear("0000:3B:00.1").unwrap().0, "0000:3b:00.1");
    for malo in ["pci0000:00", "0000:00:00", "0000:00:00.8", "zzzz:00:00.0", "0000:00:00.0x"] {
        assert!(Bdf::parsear(malo).is_none(), "{malo}");
    }
    assert_eq!(Tipo::desde_clase(0x030000), Tipo::Gpu);
    assert_eq!(Tipo::desde_clase(0x030200), Tipo::Gpu);
    assert_eq!(Tipo::desde_clase(0x060400), Tipo::Puente);
    assert_eq!(Tipo::desde_clase(0x010802), Tipo::Nvme);
    assert_eq!(Tipo::desde_clase(0x020000), Tipo::Red);
    assert_eq!(Tipo::desde_clase(0x0c0330), Tipo::Otro);
}

#[test]
fn enlaces_generacion_y_ancho_de_banda() {
    let e = Enlace::parsear("16.0 GT/s PCIe", "16").unwrap();
    assert_eq!(e.generacion(), 4);
    assert!((e.gb_s() - 31.5).abs() < 0.1, "{}", e.gb_s());
    let g1 = Enlace::parsear("2.5 GT/s", "1").unwrap();
    assert_eq!((g1.generacion(), g1.gb_s()), (1, 0.25));
    assert_eq!(Enlace::parsear("8 GT/s", "4").unwrap().generacion(), 3);
    assert!(Enlace::parsear("Unknown", "16").is_none());
    assert!(Enlace::parsear("8 GT/s", "0").is_none());
    assert_eq!(e.to_string(), "Gen4 x16 (31.5 GB/s)");
}

#[test]
fn parsea_el_sysfs_simulado() {
    let (_t, topo) = demo("parse");
    assert_eq!(topo.dispositivos.len(), 10);
    assert_eq!(topo.gpus().len(), 3);
    let g = topo.get(&b("0000:03:00.0")).unwrap();
    assert_eq!(g.vendor, 0x10de);
    assert_eq!(g.padre, Some(b("0000:02:00.0")));
    assert_eq!(g.raiz, "pci0000:00");
    assert_eq!(g.numa, Some(0));
    assert_eq!(g.driver.as_deref(), Some("gpu_drv"));
    assert_eq!(topo.ancestros(&g.bdf), vec![b("0000:02:00.0"), b("0000:01:00.0"), b("0000:00:01.0")]);
    let nic = topo.get(&b("0000:80:02.0")).unwrap();
    assert_eq!((nic.tipo, nic.driver.clone(), nic.numa), (Tipo::Red, None, Some(1)));
}

#[test]
fn detecta_enlace_degradado() {
    let (_t, topo) = demo("degradado");
    let lenta = topo.get(&b("0000:04:00.0")).unwrap();
    assert_eq!(lenta.degradado().as_deref(), Some("ancho x8 < x16"));
    assert!(topo.get(&b("0000:03:00.0")).unwrap().degradado().is_none());
    let cuello = topo.cuello_de_botella(&lenta.bdf).unwrap();
    assert!((cuello - 15.75).abs() < 0.1, "{cuello}");
}

#[test]
fn matriz_de_relaciones() {
    let (_t, topo) = demo("relaciones");
    let (g0, g1, g2) = (b("0000:03:00.0"), b("0000:04:00.0"), b("0000:81:00.0"));
    assert_eq!(topo.relacion(&g0, &g0), Relacion::Mismo);
    // g0 y g1 están bajo puertos distintos (02:00.0 y 02:01.0) del mismo switch.
    assert_eq!(topo.relacion(&g0, &g1), Relacion::Pxb);
    assert_eq!(topo.relacion(&g0, &g2), Relacion::Sys, "otro NUMA");
    assert_eq!(topo.relacion(&g0, &b("0000:00:02.0")), Relacion::Phb, "NVMe en otro puerto raíz");
    assert_eq!(topo.relacion(&b("0000:02:00.0"), &b("0000:02:01.0")), Relacion::Pix, "hermanos bajo 01:00.0");
    assert_eq!(topo.relacion(&g0, &b("0000:02:00.0")), Relacion::Pix, "padre directo");
    let m = topo.matriz_texto();
    assert!(m.contains("GPU0    X     PXB   SYS   0"), "{m}");
}

#[test]
fn mismo_numa_distinto_root_es_node() {
    let t = Tmp::new("node");
    let gpu = |cadena: &'static [&'static str], raiz: &'static str| MockDev { cadena, raiz, vendor: 0x1002, clase: 0x030000, enlace: None, numa: 0, driver: None };
    crear_sysfs_mock(&t.0, &[gpu(&["0000:00:01.0"], "pci0000:00"), gpu(&["0000:40:01.0"], "pci0000:40")]).unwrap();
    let topo = Topologia::new(leer_dispositivos(&t.0).unwrap());
    assert_eq!(topo.relacion(&b("0000:00:01.0"), &b("0000:40:01.0")), Relacion::Node);
    assert!(topo.get(&b("0000:00:01.0")).unwrap().actual.is_none(), "sin archivos de enlace");
}

#[test]
fn arbol_ordenado_y_robusto() {
    let (_t, topo) = demo("arbol");
    let a = topo.arbol();
    assert_eq!(a.len(), 10, "todos aparecen una vez");
    assert_eq!((a[0].0, a[0].1.bdf.0.as_str()), (0, "0000:00:01.0"));
    let nivel_gpu = a.iter().find(|(_, d)| d.bdf == b("0000:03:00.0")).unwrap().0;
    assert_eq!(nivel_gpu, 3);
    // Ruta inexistente → error, no pánico.
    assert!(leer_dispositivos(std::path::Path::new("/no/existe")).is_err());
    // Entradas basura en bus/pci/devices se ignoran.
    let t = Tmp::new("basura");
    std::fs::create_dir_all(t.0.join("bus/pci/devices/no-es-bdf")).unwrap();
    assert!(leer_dispositivos(&t.0).unwrap().is_empty());
}

#[test]
fn tui_con_test_backend() {
    let (_t, topo) = demo("tui");
    let mut app = App::new(topo);
    let pantalla = |app: &App| {
        let mut term = Terminal::new(TestBackend::new(110, 24)).unwrap();
        term.draw(|f| dibujar(f, app)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect::<String>()
    };
    let p = pantalla(&app);
    assert!(p.contains("Topología PCIe") && p.contains("GPU 0000:03:00.0"));
    assert!(p.contains("GPU0"));
    app.tecla('g');
    assert_eq!(app.filas().len(), 3);
    app.tecla('j');
    let p = pantalla(&app);
    assert!(p.contains("sólo GPUs"));
    assert!(p.contains("Degradado: ancho x8 < x16"), "{p}");
    for _ in 0..10 {
        app.tecla('j');
    }
    assert_eq!(app.sel, 2, "no se pasa del final");
}
