//! # Visor de procesos — lógica
//!
//! Separamos la **fuente de datos** de la **presentación** con el trait
//! [`FuenteProcesos`]: en producción se usa [`FuenteSysinfo`] (lee el SO real),
//! y en los tests una fuente falsa con datos fijos. Así ordenar, filtrar y
//! navegar es 100 % testeable y determinista.

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};
use ratatui::Frame;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

/// Foto de un proceso en un instante.
#[derive(Debug, Clone, PartialEq)]
pub struct InfoProceso {
    pub pid: u32,
    pub nombre: String,
    /// % de CPU (puede superar 100 en multi-núcleo, como en `top`).
    pub cpu: f32,
    /// Memoria residente en bytes.
    pub memoria: u64,
}

/// Abstracción de "algo que sabe listar procesos".
pub trait FuenteProcesos {
    /// Refresca y devuelve la lista actual.
    fn listar(&mut self) -> Vec<InfoProceso>;
    /// Memoria total del sistema (para calcular porcentajes).
    fn memoria_total(&self) -> u64;
}

/// Fuente real basada en `sysinfo`.
pub struct FuenteSysinfo {
    sys: System,
}

impl Default for FuenteSysinfo {
    fn default() -> Self {
        Self::new()
    }
}

impl FuenteSysinfo {
    pub fn new() -> Self {
        // Solo pedimos lo necesario (procesos + memoria) para ser livianos.
        let mut sys = System::new_with_specifics(RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()));
        sys.refresh_memory();
        FuenteSysinfo { sys }
    }
}

impl FuenteProcesos for FuenteSysinfo {
    fn listar(&mut self) -> Vec<InfoProceso> {
        // El % de CPU se calcula como diferencia entre dos refrescos: la primera
        // lectura siempre da 0 y se estabiliza desde la segunda.
        self.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::everything());
        self.sys
            .processes()
            .iter()
            .map(|(pid, p)| InfoProceso {
                pid: pid.as_u32(),
                nombre: p.name().to_string_lossy().into_owned(),
                cpu: p.cpu_usage(),
                memoria: p.memory(),
            })
            .collect()
    }
    fn memoria_total(&self) -> u64 {
        self.sys.total_memory()
    }
}

/// Criterio de orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orden {
    Cpu,
    Memoria,
    Pid,
    Nombre,
}

impl Orden {
    /// Rota entre criterios con la tecla `s`.
    pub fn siguiente(self) -> Self {
        match self {
            Orden::Cpu => Orden::Memoria,
            Orden::Memoria => Orden::Pid,
            Orden::Pid => Orden::Nombre,
            Orden::Nombre => Orden::Cpu,
        }
    }
    pub fn etiqueta(self) -> &'static str {
        match self {
            Orden::Cpu => "CPU",
            Orden::Memoria => "MEM",
            Orden::Pid => "PID",
            Orden::Nombre => "NOMBRE",
        }
    }
}

/// Ordena en el lugar. CPU y memoria van de mayor a menor (lo interesante arriba).
pub fn ordenar(v: &mut [InfoProceso], o: Orden) {
    match o {
        Orden::Cpu => v.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(a.pid.cmp(&b.pid))),
        Orden::Memoria => v.sort_by(|a, b| b.memoria.cmp(&a.memoria).then(a.pid.cmp(&b.pid))),
        Orden::Pid => v.sort_by_key(|p| p.pid),
        Orden::Nombre => v.sort_by(|a, b| a.nombre.to_lowercase().cmp(&b.nombre.to_lowercase()).then(a.pid.cmp(&b.pid))),
    }
}

/// Filtra por subcadena del nombre (sin distinguir mayúsculas) o por PID exacto.
pub fn filtrar(v: Vec<InfoProceso>, q: &str) -> Vec<InfoProceso> {
    if q.is_empty() {
        return v;
    }
    let ql = q.to_lowercase();
    v.into_iter().filter(|p| p.nombre.to_lowercase().contains(&ql) || p.pid.to_string() == q).collect()
}

/// Estado de la TUI.
pub struct App<F: FuenteProcesos> {
    pub fuente: F,
    pub procesos: Vec<InfoProceso>,
    pub orden: Orden,
    pub filtro: String,
    /// `true` mientras se está escribiendo el filtro (tecla `/`).
    pub editando: bool,
    pub tabla: TableState,
    pub salir: bool,
}

impl<F: FuenteProcesos> App<F> {
    pub fn new(fuente: F) -> Self {
        let mut a = App { fuente, procesos: vec![], orden: Orden::Cpu, filtro: String::new(), editando: false, tabla: TableState::default(), salir: false };
        a.refrescar();
        a
    }

    /// Relee la fuente y reaplica filtro + orden, conservando la selección acotada.
    pub fn refrescar(&mut self) {
        let mut v = filtrar(self.fuente.listar(), &self.filtro);
        ordenar(&mut v, self.orden);
        self.procesos = v;
        let sel = self.tabla.selected().unwrap_or(0).min(self.procesos.len().saturating_sub(1));
        self.tabla.select(if self.procesos.is_empty() { None } else { Some(sel) });
    }

    /// Procesa una tecla. En modo edición, las letras van al filtro.
    pub fn tecla(&mut self, c: char) {
        if self.editando {
            match c {
                '\n' | '\x1b' => self.editando = false,
                '\x08' => {
                    self.filtro.pop();
                }
                c => self.filtro.push(c),
            }
            self.tabla.select(Some(0));
            self.refrescar();
            return;
        }
        match c {
            'q' => self.salir = true,
            'j' => {
                let n = self.procesos.len();
                if n > 0 {
                    self.tabla.select(Some((self.tabla.selected().unwrap_or(0) + 1).min(n - 1)));
                }
            }
            'k' => self.tabla.select(Some(self.tabla.selected().unwrap_or(0).saturating_sub(1))),
            's' => {
                self.orden = self.orden.siguiente();
                self.refrescar();
            }
            'c' => {
                self.orden = Orden::Cpu;
                self.refrescar();
            }
            'm' => {
                self.orden = Orden::Memoria;
                self.refrescar();
            }
            '/' => self.editando = true,
            'r' => self.refrescar(),
            _ => {}
        }
    }
}

/// Memoria en MiB con un decimal.
pub fn mib(b: u64) -> String {
    format!("{:.1}", b as f64 / 1_048_576.0)
}

/// Dibuja tabla + barra de estado.
pub fn dibujar<F: FuenteProcesos>(f: &mut Frame, app: &mut App<F>) {
    let z = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(3), Constraint::Length(1)]).split(f.area());
    let total = app.fuente.memoria_total().max(1);
    let filas = app.procesos.iter().map(|p| {
        Row::new(vec![
            p.pid.to_string(),
            format!("{:.1}", p.cpu),
            mib(p.memoria),
            format!("{:.1}", p.memoria as f64 * 100.0 / total as f64),
            p.nombre.clone(),
        ])
    });
    let enc = Row::new(vec!["PID", "CPU%", "MEM MiB", "MEM%", "NOMBRE"]).style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow));
    let t = Table::new(filas, [Constraint::Length(8), Constraint::Length(7), Constraint::Length(9), Constraint::Length(6), Constraint::Min(10)])
        .header(enc)
        .block(Block::default().borders(Borders::ALL).title(format!(" Procesos ({}) · orden: {} ", app.procesos.len(), app.orden.etiqueta())))
        .row_highlight_style(Style::default().bg(Color::Blue).fg(Color::White));
    f.render_stateful_widget(t, z[0], &mut app.tabla);
    let estado = if app.editando {
        format!("Filtro: {}▏ (Enter para terminar)", app.filtro)
    } else {
        format!("j/k mover · s rotar orden · c CPU · m MEM · / filtrar [{}] · r refrescar · q salir", app.filtro)
    };
    f.render_widget(Paragraph::new(estado), z[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    /// Fuente falsa con datos fijos.
    struct Falsa(Vec<InfoProceso>);
    impl FuenteProcesos for Falsa {
        fn listar(&mut self) -> Vec<InfoProceso> {
            self.0.clone()
        }
        fn memoria_total(&self) -> u64 {
            1000
        }
    }

    fn p(pid: u32, n: &str, cpu: f32, mem: u64) -> InfoProceso {
        InfoProceso { pid, nombre: n.into(), cpu, memoria: mem }
    }

    fn fuente() -> Falsa {
        Falsa(vec![p(10, "bash", 0.5, 100), p(2, "Firefox", 40.0, 500), p(33, "cargo", 90.0, 300)])
    }

    #[test]
    fn ordena_por_criterios() {
        let mut v = fuente().0;
        ordenar(&mut v, Orden::Cpu);
        assert_eq!(v[0].nombre, "cargo");
        ordenar(&mut v, Orden::Memoria);
        assert_eq!(v[0].nombre, "Firefox");
        ordenar(&mut v, Orden::Pid);
        assert_eq!(v[0].pid, 2);
        ordenar(&mut v, Orden::Nombre);
        assert_eq!(v[0].nombre, "bash");
    }

    #[test]
    fn filtra_por_nombre_y_pid() {
        assert_eq!(filtrar(fuente().0, "fire").len(), 1);
        assert_eq!(filtrar(fuente().0, "33")[0].nombre, "cargo");
        assert_eq!(filtrar(fuente().0, "").len(), 3);
    }

    #[test]
    fn teclas_y_modo_filtro() {
        let mut a = App::new(fuente());
        assert_eq!(a.procesos[0].nombre, "cargo");
        a.tecla('m');
        assert_eq!(a.procesos[0].nombre, "Firefox");
        a.tecla('/');
        for c in "ba".chars() {
            a.tecla(c);
        }
        a.tecla('\n');
        assert_eq!(a.procesos.len(), 1);
        a.tecla('j');
        assert_eq!(a.tabla.selected(), Some(0));
        a.tecla('s');
        assert_eq!(a.orden, Orden::Pid);
        a.tecla('q');
        assert!(a.salir);
    }

    #[test]
    fn render_headless() {
        let mut a = App::new(fuente());
        let mut t = Terminal::new(TestBackend::new(80, 10)).unwrap();
        t.draw(|f| dibujar(f, &mut a)).unwrap();
        let s: String = t.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(s.contains("cargo") && s.contains("orden: CPU"));
    }

    /// Prueba con el sistema real: debe haber al menos un proceso (este test).
    #[test]
    fn fuente_real_lista_procesos() {
        let mut f = FuenteSysinfo::new();
        let v = f.listar();
        assert!(!v.is_empty());
        assert!(v.iter().any(|p| p.pid == std::process::id()));
    }
}
