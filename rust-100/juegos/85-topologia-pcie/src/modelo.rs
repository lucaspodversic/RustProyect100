//! Modelo de un dispositivo PCI y utilidades de enlace PCIe.

use std::fmt;

/// Dirección PCI "dominio:bus:dispositivo.función", p. ej. `0000:01:00.0`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bdf(pub String);

impl Bdf {
    /// Valida el formato `dddd:bb:dd.f` (hex). Así distinguimos, dentro de
    /// una ruta de sysfs, qué componentes son dispositivos y cuáles no.
    pub fn parsear(s: &str) -> Option<Bdf> {
        let b = s.as_bytes();
        let hex = |r: std::ops::Range<usize>| b[r].iter().all(|c| c.is_ascii_hexdigit());
        (b.len() == 12 && b[4] == b':' && b[7] == b':' && b[10] == b'.' && hex(0..4) && hex(5..7) && hex(8..10) && (b'0'..=b'7').contains(&b[11])).then(|| Bdf(s.to_ascii_lowercase()))
    }
}

impl fmt::Display for Bdf {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Gpu,
    Puente,
    Nvme,
    Red,
    Otro,
}

impl Tipo {
    /// Clasificación por código de clase PCI (24 bits: clase/subclase/interfaz).
    pub fn desde_clase(clase: u32) -> Tipo {
        match clase >> 8 {
            0x0300 | 0x0302 | 0x0380 => Tipo::Gpu, // VGA, 3D, otro display
            0x0604 => Tipo::Puente,                // puente PCI-PCI (puertos de switch/root)
            0x0108 => Tipo::Nvme,
            0x0200 | 0x0207 => Tipo::Red,          // Ethernet, InfiniBand
            0x1200 => Tipo::Gpu,                   // aceleradores de procesamiento
            _ => Tipo::Otro,
        }
    }
    pub fn icono(self) -> &'static str {
        match self {
            Tipo::Gpu => "GPU",
            Tipo::Puente => "BR ",
            Tipo::Nvme => "NVM",
            Tipo::Red => "NIC",
            Tipo::Otro => "   ",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enlace {
    /// Velocidad por carril en GT/s (2.5, 5, 8, 16, 32, 64).
    pub gts: f32,
    pub ancho: u32,
}

impl Enlace {
    /// Parsea los formatos de sysfs: "16.0 GT/s PCIe", "8 GT/s", "2.5 GT/s".
    pub fn parsear(vel: &str, ancho: &str) -> Option<Enlace> {
        let gts = vel.split_whitespace().next()?.parse::<f32>().ok().filter(|v| *v > 0.0)?;
        let ancho = ancho.trim().parse::<u32>().ok().filter(|w| *w > 0)?;
        Some(Enlace { gts, ancho })
    }

    /// Generación PCIe aproximada según la velocidad.
    pub fn generacion(&self) -> u8 {
        match self.gts {
            v if v <= 2.5 => 1,
            v if v <= 5.0 => 2,
            v if v <= 8.0 => 3,
            v if v <= 16.0 => 4,
            v if v <= 32.0 => 5,
            _ => 6,
        }
    }

    /// Ancho de banda útil por sentido en GB/s descontando la codificación:
    /// Gen1–2 usan 8b/10b (80 %), Gen3–5 128b/130b (~98.5 %), Gen6 FLIT (~94 %).
    pub fn gb_s(&self) -> f32 {
        let eficiencia = match self.generacion() {
            1 | 2 => 0.8,
            3..=5 => 128.0 / 130.0,
            _ => 242.0 / 256.0,
        };
        self.gts * eficiencia * self.ancho as f32 / 8.0
    }
}

impl fmt::Display for Enlace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Gen{} x{} ({:.1} GB/s)", self.generacion(), self.ancho, self.gb_s())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dispositivo {
    pub bdf: Bdf,
    pub vendor: u16,
    pub device: u16,
    pub clase: u32,
    pub tipo: Tipo,
    pub actual: Option<Enlace>,
    pub maximo: Option<Enlace>,
    pub numa: Option<i32>,
    pub driver: Option<String>,
    /// Puente padre (otro BDF) o `None` si cuelga directo del root complex.
    pub padre: Option<Bdf>,
    /// Root complex: componente `pciDDDD:BB` de la ruta canónica.
    pub raiz: String,
}

impl Dispositivo {
    /// ¿El enlace negoció por debajo de su capacidad? (problema típico:
    /// GPU x16 en slot x8, o placa que cae a Gen1 por ahorro de energía).
    pub fn degradado(&self) -> Option<String> {
        let (a, m) = (self.actual?, self.maximo?);
        let mut v = vec![];
        if a.gts < m.gts {
            v.push(format!("velocidad {} < {} GT/s", a.gts, m.gts));
        }
        if a.ancho < m.ancho {
            v.push(format!("ancho x{} < x{}", a.ancho, m.ancho));
        }
        (!v.is_empty()).then(|| v.join(", "))
    }

    pub fn nombre_vendor(&self) -> &'static str {
        // Tabla mínima de IDs públicos del registro PCI-SIG (uso descriptivo).
        match self.vendor {
            0x10de => "NVIDIA",
            0x1002 => "AMD",
            0x8086 => "Intel",
            0x15b3 => "Mellanox",
            0x144d => "Samsung",
            0x1af4 => "Virtio",
            0x1b36 => "QEMU",
            0x1d0f => "Amazon",
            _ => "desconocido",
        }
    }
}
