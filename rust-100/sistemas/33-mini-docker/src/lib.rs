//! # Mini Docker (educativo)
//!
//! Runtime de contenedores **simulado en userspace**. En Linux real se
//! usaría `clone(CLONE_NEWNS|CLONE_NEWPID|…)` + cgroups v2; aquí modelamos
//! las mismas abstracciones para poder probarlas sin privilegios:
//!
//! - Namespaces: pid / mount / net / uts / user / ipc (como conjuntos).
//! - Cgroups: límites de CPU (cuota %) y memoria (bytes) + contadores.
//! - OverlayFS: capas "lower" + "upper" con escritura copy-on-write.
//! - Runtime: ciclo de vida create → start → stop → remove.
//!
//! El código del path real en Linux está detrás del feature `linux` (ver
//! `linux.rs`) y se documenta; los tests usan siempre el simulador.

pub mod ns {
    //! Namespaces como conjuntos de identificadores aislados.
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(1);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Kind {
        Pid,
        Mount,
        Net,
        Uts,
        User,
        Ipc,
    }

    /// Un namespace: un "mundo" con su propio espacio de nombres.
    #[derive(Debug, Clone)]
    pub struct Namespace {
        pub id: u64,
        pub kind: Kind,
        /// Contenido: PIDs, rutas montadas, interfaces, hostname, etc.
        pub entries: HashMap<String, String>,
    }

    impl Namespace {
        pub fn new(kind: Kind) -> Self {
            Self { id: NEXT.fetch_add(1, Ordering::Relaxed), kind, entries: HashMap::new() }
        }
    }

    /// Set de namespaces de un contenedor (como `/proc/<pid>/ns`).
    #[derive(Debug, Clone)]
    pub struct NsSet {
        pub pid: Namespace,
        pub mount: Namespace,
        pub net: Namespace,
        pub uts: Namespace,
        pub user: Namespace,
        pub ipc: Namespace,
    }

    impl NsSet {
        /// Crea un set fresco (un contenedor nuevo).
        pub fn fresh(hostname: &str) -> Self {
            let mut uts = Namespace::new(Kind::Uts);
            uts.entries.insert("hostname".into(), hostname.into());
            Self {
                pid: Namespace::new(Kind::Pid),
                mount: Namespace::new(Kind::Mount),
                net: Namespace::new(Kind::Net),
                uts,
                user: Namespace::new(Kind::User),
                ipc: Namespace::new(Kind::Ipc),
            }
        }

        /// Comparte el netns con otro contenedor (como `--network=container:X`).
        pub fn share_net(&mut self, other: &NsSet) {
            self.net = other.net.clone();
        }
    }
}

pub mod cgroup {
    //! Contabilidad y límites (CPU + memoria). En Linux real: cgroup v2
    //! con `cpu.max` y `memory.max` escritos en `/sys/fs/cgroup/...`.

    #[derive(Debug, Clone)]
    pub struct Limits {
        /// Porcentaje de CPU permitido (100 = 1 núcleo completo).
        pub cpu_percent: u32,
        /// Tope de memoria en bytes.
        pub memory_bytes: u64,
    }

    impl Default for Limits {
        fn default() -> Self {
            Self { cpu_percent: 100, memory_bytes: 256 * 1024 * 1024 }
        }
    }

    #[derive(Debug, Default, Clone)]
    pub struct Usage {
        pub cpu_ms: u64,
        pub memory_bytes: u64,
    }

    #[derive(Debug, Clone)]
    pub struct Cgroup {
        pub name: String,
        pub limits: Limits,
        pub usage: Usage,
    }

    impl Cgroup {
        pub fn new(name: &str, limits: Limits) -> Self {
            Self { name: name.into(), limits, usage: Usage::default() }
        }

        /// ¿Puede el proceso consumir `extra` bytes de RAM?
        pub fn admit_memory(&self, extra: u64) -> bool {
            self.usage.memory_bytes.saturating_add(extra) <= self.limits.memory_bytes
        }

        /// ¿Puede el proceso correr `extra_ms` de CPU en esta ventana?
        /// (Modelo simplificado: cuota = cpu_percent * 10 ms por "tick" de 1 s.)
        pub fn admit_cpu(&self, extra_ms: u64, window_ms: u64) -> bool {
            let quota = (self.limits.cpu_percent as u64).saturating_mul(window_ms) / 100;
            self.usage.cpu_ms.saturating_add(extra_ms) <= quota
        }

        pub fn charge_memory(&mut self, bytes: u64) -> Result<(), String> {
            if !self.admit_memory(bytes) {
                return Err(format!("OOM: {} supera memory.max={}", self.name, self.limits.memory_bytes));
            }
            self.usage.memory_bytes += bytes;
            Ok(())
        }

        pub fn charge_cpu(&mut self, ms: u64, window_ms: u64) -> Result<(), String> {
            if !self.admit_cpu(ms, window_ms) {
                return Err(format!("throttled: {} supera cpu.max", self.name));
            }
            self.usage.cpu_ms += ms;
            Ok(())
        }

        pub fn reset_window(&mut self) {
            self.usage.cpu_ms = 0;
        }
    }
}

pub mod overlay {
    //! OverlayFS simulado: capas lower (solo lectura) + upper (escritura).
    //! Una lectura mira primero upper; si no, recorre lower de arriba a abajo.
    //! Una escritura siempre va al upper (copy-on-write).
    use std::collections::HashMap;

    #[derive(Debug, Default, Clone)]
    pub struct Layer {
        pub files: HashMap<String, Vec<u8>>,
        /// Archivos "borrados" (whiteout), como en OverlayFS real.
        pub whiteouts: Vec<String>,
    }

    #[derive(Debug, Default)]
    pub struct Overlay {
        pub lowers: Vec<Layer>,
        pub upper: Layer,
    }

    impl Overlay {
        pub fn read(&self, path: &str) -> Option<Vec<u8>> {
            if self.upper.whiteouts.iter().any(|w| w == path) {
                return None;
            }
            if let Some(v) = self.upper.files.get(path) {
                return Some(v.clone());
            }
            for layer in self.lowers.iter().rev() {
                if layer.whiteouts.iter().any(|w| w == path) {
                    return None;
                }
                if let Some(v) = layer.files.get(path) {
                    return Some(v.clone());
                }
            }
            None
        }

        pub fn write(&mut self, path: &str, data: Vec<u8>) {
            self.upper.whiteouts.retain(|w| w != path);
            self.upper.files.insert(path.into(), data);
        }

        pub fn remove(&mut self, path: &str) {
            self.upper.files.remove(path);
            if !self.upper.whiteouts.iter().any(|w| w == path) {
                self.upper.whiteouts.push(path.into());
            }
        }

        pub fn list(&self) -> Vec<String> {
            let mut names = std::collections::BTreeSet::new();
            for layer in &self.lowers {
                names.extend(layer.files.keys().cloned());
                for w in &layer.whiteouts {
                    names.remove(w);
                }
            }
            names.extend(self.upper.files.keys().cloned());
            for w in &self.upper.whiteouts {
                names.remove(w);
            }
            names.into_iter().collect()
        }
    }
}

pub mod runtime {
    //! Ciclo de vida de un contenedor.
    use crate::cgroup::{Cgroup, Limits};
    use crate::ns::NsSet;
    use crate::overlay::{Layer, Overlay};
    use std::collections::HashMap;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum State {
        Created,
        Running,
        Stopped,
        Removed,
    }

    #[derive(Debug)]
    pub struct Container {
        pub id: String,
        pub image: String,
        pub cmd: Vec<String>,
        pub state: State,
        pub ns: NsSet,
        pub cgroup: Cgroup,
        pub rootfs: Overlay,
        pub env: HashMap<String, String>,
    }

    #[derive(Debug, Default)]
    pub struct Runtime {
        containers: HashMap<String, Container>,
        /// Imágenes = capas lower prearmadas.
        images: HashMap<String, Layer>,
    }

    impl Runtime {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn add_image(&mut self, name: &str, files: Vec<(&str, &[u8])>) {
            let mut layer = Layer::default();
            for (p, d) in files {
                layer.files.insert(p.into(), d.to_vec());
            }
            self.images.insert(name.into(), layer);
        }

        pub fn create(
            &mut self,
            id: &str,
            image: &str,
            cmd: Vec<String>,
            limits: Limits,
        ) -> Result<(), String> {
            if self.containers.contains_key(id) {
                return Err("id ya existe".into());
            }
            let lower = self.images.get(image).cloned().ok_or("imagen inexistente")?;
            let mut rootfs = Overlay::default();
            rootfs.lowers.push(lower);
            let c = Container {
                id: id.into(),
                image: image.into(),
                cmd,
                state: State::Created,
                ns: NsSet::fresh(id),
                cgroup: Cgroup::new(id, limits),
                rootfs,
                env: HashMap::new(),
            };
            self.containers.insert(id.into(), c);
            Ok(())
        }

        pub fn start(&mut self, id: &str) -> Result<(), String> {
            let c = self.containers.get_mut(id).ok_or("no existe")?;
            if c.state != State::Created && c.state != State::Stopped {
                return Err(format!("estado inválido: {:?}", c.state));
            }
            // PID 1 del contenedor (en un ns de PID propio).
            c.ns.pid.entries.insert("1".into(), c.cmd.join(" "));
            c.state = State::Running;
            Ok(())
        }

        pub fn stop(&mut self, id: &str) -> Result<(), String> {
            let c = self.containers.get_mut(id).ok_or("no existe")?;
            if c.state != State::Running {
                return Err("no está running".into());
            }
            c.state = State::Stopped;
            Ok(())
        }

        pub fn remove(&mut self, id: &str) -> Result<(), String> {
            let c = self.containers.get_mut(id).ok_or("no existe")?;
            if c.state == State::Running {
                return Err("detener primero".into());
            }
            c.state = State::Removed;
            self.containers.remove(id);
            Ok(())
        }

        pub fn get(&self, id: &str) -> Option<&Container> {
            self.containers.get(id)
        }

        pub fn get_mut(&mut self, id: &str) -> Option<&mut Container> {
            self.containers.get_mut(id)
        }

        pub fn list(&self) -> Vec<&Container> {
            let mut v: Vec<_> = self.containers.values().collect();
            v.sort_by_key(|c| &c.id);
            v
        }
    }
}

/// Documentación del path real en Linux (no se ejecuta en tests).
/// Activar con `--features linux` en un host privilegiado.
#[cfg(feature = "linux")]
pub mod linux {
    //! Pseudocódigo del camino real:
    //! 1. `unshare(CLONE_NEWNS|CLONE_NEWPID|CLONE_NEWNET|CLONE_NEWUTS|CLONE_NEWIPC)`
    //! 2. montar overlay: `mount("overlay", root, "overlay", 0, "lowerdir=…,upperdir=…,workdir=…")`
    //! 3. `pivot_root(new_root, put_old)` + `umount2(put_old, MNT_DETACH)`
    //! 4. escribir límites en `/sys/fs/cgroup/<name>/memory.max` y `cpu.max`
    //! 5. `execve(cmd[0], cmd, env)`
    //!
    //! Requiere `CAP_SYS_ADMIN` (o root). Este crate NO lo ejecuta: sirve
    //! como referencia pedagógica junto al simulador de userspace.
}

#[cfg(test)]
mod tests {
    use super::cgroup::Limits;
    use super::overlay::{Layer, Overlay};
    use super::runtime::{Runtime, State};

    #[test]
    fn overlay_cow_y_whiteout() {
        let mut o = Overlay::default();
        let mut base = Layer::default();
        base.files.insert("/bin/sh".into(), b"busybox".to_vec());
        base.files.insert("/etc/hosts".into(), b"127.0.0.1".to_vec());
        o.lowers.push(base);
        assert_eq!(o.read("/bin/sh"), Some(b"busybox".to_vec()));
        o.write("/etc/hosts", b"10.0.0.1".to_vec());
        assert_eq!(o.read("/etc/hosts"), Some(b"10.0.0.1".to_vec()));
        o.remove("/bin/sh");
        assert_eq!(o.read("/bin/sh"), None);
        assert_eq!(o.list(), vec!["/etc/hosts".to_string()]);
    }

    #[test]
    fn cgroup_oom_y_throttle() {
        let mut cg = super::cgroup::Cgroup::new(
            "c1",
            Limits { cpu_percent: 50, memory_bytes: 1024 },
        );
        assert!(cg.charge_memory(512).is_ok());
        assert!(cg.charge_memory(600).is_err());
        assert!(cg.charge_cpu(400, 1000).is_ok()); // 50% de 1000ms = 500
        assert!(cg.charge_cpu(200, 1000).is_err());
        cg.reset_window();
        assert!(cg.charge_cpu(100, 1000).is_ok());
    }

    #[test]
    fn ciclo_de_vida() {
        let mut rt = Runtime::new();
        rt.add_image("alpine", vec![("/bin/sh", b"ash"), ("/etc/os-release", b"Alpine")]);
        rt.create(
            "web",
            "alpine",
            vec!["/bin/sh".into()],
            Limits { cpu_percent: 100, memory_bytes: 64 * 1024 * 1024 },
        )
        .unwrap();
        assert_eq!(rt.get("web").unwrap().state, State::Created);
        assert_eq!(rt.get("web").unwrap().ns.uts.entries["hostname"], "web");
        rt.start("web").unwrap();
        assert_eq!(rt.get("web").unwrap().state, State::Running);
        assert_eq!(rt.get("web").unwrap().ns.pid.entries["1"], "/bin/sh");
        // Copy-on-write dentro del contenedor.
        rt.get_mut("web").unwrap().rootfs.write("/tmp/x", b"hola".to_vec());
        assert_eq!(rt.get("web").unwrap().rootfs.read("/tmp/x"), Some(b"hola".to_vec()));
        assert_eq!(rt.get("web").unwrap().rootfs.read("/bin/sh"), Some(b"ash".to_vec()));
        assert!(rt.remove("web").is_err()); // todavía running
        rt.stop("web").unwrap();
        rt.remove("web").unwrap();
        assert!(rt.get("web").is_none());
    }

    #[test]
    fn compartir_netns() {
        let mut rt = Runtime::new();
        rt.add_image("base", vec![]);
        rt.create("a", "base", vec![], Limits::default()).unwrap();
        rt.create("b", "base", vec![], Limits::default()).unwrap();
        let a_net = rt.get("a").unwrap().ns.net.id;
        {
            let a_ns = rt.get("a").unwrap().ns.clone();
            rt.get_mut("b").unwrap().ns.share_net(&a_ns);
        }
        assert_eq!(rt.get("b").unwrap().ns.net.id, a_net);
        assert_ne!(rt.get("b").unwrap().ns.pid.id, rt.get("a").unwrap().ns.pid.id);
    }
}
