//! Lectura de `/sys/bus/pci/devices` (o de una raíz alternativa para tests).
//!
//! Cada entrada es un symlink a la ruta canónica del dispositivo, por ej.:
//! `/sys/devices/pci0000:00/0000:00:01.0/0000:01:00.0`. Los componentes que
//! son BDF forman la cadena de puentes: el anteúltimo BDF es el padre.

use crate::modelo::{Bdf, Dispositivo, Enlace, Tipo};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ErrorSysfs {
    #[error("no se pudo leer {0}: {1}")]
    Io(PathBuf, std::io::Error),
}

fn leer(dir: &Path, archivo: &str) -> Option<String> {
    fs::read_to_string(dir.join(archivo)).ok().map(|s| s.trim().to_string())
}

fn hex(s: Option<String>) -> Option<u32> {
    let s = s?;
    u32::from_str_radix(s.trim_start_matches("0x"), 16).ok()
}

/// Lee todos los dispositivos bajo `<raiz>/bus/pci/devices`.
/// Archivos faltantes o ilegibles no abortan: el campo queda en `None`
/// (en máquinas reales muchos dispositivos no exponen `current_link_*`).
pub fn leer_dispositivos(raiz: &Path) -> Result<Vec<Dispositivo>, ErrorSysfs> {
    let base = raiz.join("bus/pci/devices");
    let entradas = fs::read_dir(&base).map_err(|e| ErrorSysfs::Io(base.clone(), e))?;
    let mut v = Vec::new();
    for e in entradas.flatten() {
        let nombre = e.file_name().to_string_lossy().to_string();
        let Some(bdf) = Bdf::parsear(&nombre) else { continue };
        // Ruta canónica (sigue el symlink); si falla, usamos la entrada tal cual.
        let real = fs::canonicalize(e.path()).unwrap_or_else(|_| e.path());
        let comps: Vec<String> = real.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
        let cadena: Vec<Bdf> = comps.iter().filter_map(|c| Bdf::parsear(c)).collect();
        let padre = if cadena.len() >= 2 { Some(cadena[cadena.len() - 2].clone()) } else { None };
        let raiz_c = comps.iter().find(|c| c.starts_with("pci") && c.contains(':')).cloned().unwrap_or_else(|| "pci?".into());
        let clase = hex(leer(&real, "class")).unwrap_or(0);
        v.push(Dispositivo {
            vendor: hex(leer(&real, "vendor")).unwrap_or(0) as u16,
            device: hex(leer(&real, "device")).unwrap_or(0) as u16,
            clase,
            tipo: Tipo::desde_clase(clase),
            actual: Enlace::parsear(&leer(&real, "current_link_speed").unwrap_or_default(), &leer(&real, "current_link_width").unwrap_or_default()),
            maximo: Enlace::parsear(&leer(&real, "max_link_speed").unwrap_or_default(), &leer(&real, "max_link_width").unwrap_or_default()),
            numa: leer(&real, "numa_node").and_then(|s| s.parse().ok()).filter(|n: &i32| *n >= 0),
            driver: fs::read_link(real.join("driver")).ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())),
            padre,
            raiz: raiz_c,
            bdf,
        });
    }
    v.sort_by(|a, b| a.bdf.cmp(&b.bdf));
    Ok(v)
}

/// Descripción de un dispositivo para el generador de sysfs simulado.
#[derive(Debug, Clone)]
pub struct MockDev<'a> {
    /// Cadena de BDF desde el root complex, el último es el dispositivo.
    pub cadena: &'a [&'a str],
    pub raiz: &'a str,
    pub vendor: u16,
    pub clase: u32,
    pub enlace: Option<(&'a str, u32, &'a str, u32)>, // actual vel, ancho, max vel, ancho
    pub numa: i32,
    pub driver: Option<&'a str>,
}

/// Crea un árbol sysfs falso en `dir` (estructura real + symlinks), para
/// tests y para la demo en máquinas sin GPUs.
pub fn crear_sysfs_mock(dir: &Path, devs: &[MockDev]) -> std::io::Result<()> {
    let bus = dir.join("bus/pci/devices");
    fs::create_dir_all(&bus)?;
    fs::create_dir_all(dir.join("bus/pci/drivers"))?;
    for d in devs {
        let mut ruta = dir.join("devices").join(d.raiz);
        for c in d.cadena {
            ruta = ruta.join(c);
        }
        fs::create_dir_all(&ruta)?;
        fs::write(ruta.join("vendor"), format!("0x{:04x}\n", d.vendor))?;
        fs::write(ruta.join("device"), "0x1234\n")?;
        fs::write(ruta.join("class"), format!("0x{:06x}\n", d.clase))?;
        fs::write(ruta.join("numa_node"), format!("{}\n", d.numa))?;
        if let Some((va, wa, vm, wm)) = d.enlace {
            fs::write(ruta.join("current_link_speed"), format!("{va}\n"))?;
            fs::write(ruta.join("current_link_width"), format!("{wa}\n"))?;
            fs::write(ruta.join("max_link_speed"), format!("{vm}\n"))?;
            fs::write(ruta.join("max_link_width"), format!("{wm}\n"))?;
        }
        if let Some(drv) = d.driver {
            let dd = dir.join("bus/pci/drivers").join(drv);
            fs::create_dir_all(&dd)?;
            let _ = fs::remove_file(ruta.join("driver"));
            std::os::unix::fs::symlink(&dd, ruta.join("driver"))?;
        }
        let enlace = bus.join(d.cadena.last().unwrap());
        let _ = fs::remove_file(&enlace);
        std::os::unix::fs::symlink(&ruta, enlace)?;
    }
    Ok(())
}

/// Servidor ficticio: 2 root complex (NUMA 0 y 1), switches PCIe con GPUs,
/// una GPU degradada a x8, un NVMe y una NIC.
pub fn sysfs_demo(dir: &Path) -> std::io::Result<()> {
    const G4: &str = "16.0 GT/s PCIe";
    let gpu = |cadena: &'static [&'static str], raiz: &'static str, numa, enlace| MockDev { cadena, raiz, vendor: 0x10de, clase: 0x030200, enlace: Some(enlace), numa, driver: Some("gpu_drv") };
    let br = |cadena: &'static [&'static str], raiz: &'static str, numa| MockDev { cadena, raiz, vendor: 0x8086, clase: 0x060400, enlace: Some((G4, 16, G4, 16)), numa, driver: Some("pcieport") };
    crear_sysfs_mock(dir, &[
        br(&["0000:00:01.0"], "pci0000:00", 0),
        br(&["0000:00:01.0", "0000:01:00.0"], "pci0000:00", 0),
        br(&["0000:00:01.0", "0000:01:00.0", "0000:02:00.0"], "pci0000:00", 0),
        br(&["0000:00:01.0", "0000:01:00.0", "0000:02:01.0"], "pci0000:00", 0),
        gpu(&["0000:00:01.0", "0000:01:00.0", "0000:02:00.0", "0000:03:00.0"], "pci0000:00", 0, (G4, 16, G4, 16)),
        gpu(&["0000:00:01.0", "0000:01:00.0", "0000:02:01.0", "0000:04:00.0"], "pci0000:00", 0, (G4, 8, G4, 16)),
        MockDev { cadena: &["0000:00:02.0"], raiz: "pci0000:00", vendor: 0x144d, clase: 0x010802, enlace: Some(("8.0 GT/s PCIe", 4, "8.0 GT/s PCIe", 4)), numa: 0, driver: Some("nvme") },
        br(&["0000:80:01.0"], "pci0000:80", 1),
        gpu(&["0000:80:01.0", "0000:81:00.0"], "pci0000:80", 1, (G4, 16, G4, 16)),
        MockDev { cadena: &["0000:80:02.0"], raiz: "pci0000:80", vendor: 0x15b3, clase: 0x020700, enlace: Some((G4, 16, G4, 16)), numa: 1, driver: None },
    ])
}
