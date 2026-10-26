//! TUI: árbol a la izquierda, detalle y matriz de GPUs a la derecha.

use crate::modelo::Tipo;
use crate::topologia::Topologia;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

pub struct App {
    pub topo: Topologia,
    pub sel: usize,
    pub solo_gpus: bool,
}

impl App {
    pub fn new(topo: Topologia) -> Self {
        Self { topo, sel: 0, solo_gpus: false }
    }

    /// Filas visibles: (nivel, índice en `dispositivos`).
    pub fn filas(&self) -> Vec<(usize, usize)> {
        let pos = |b: &crate::modelo::Bdf| self.topo.dispositivos.iter().position(|d| &d.bdf == b).unwrap();
        self.topo.arbol().into_iter().filter(|(_, d)| !self.solo_gpus || d.tipo == Tipo::Gpu).map(|(n, d)| (n, pos(&d.bdf))).collect()
    }

    pub fn tecla(&mut self, c: char) {
        let n = self.filas().len();
        match c {
            'j' => self.sel = (self.sel + 1).min(n.saturating_sub(1)),
            'k' => self.sel = self.sel.saturating_sub(1),
            'g' => {
                self.solo_gpus = !self.solo_gpus;
                self.sel = 0;
            }
            _ => {}
        }
    }
}

pub fn dibujar(f: &mut Frame, app: &App) {
    let [izq, der] = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(f.area());
    let [det, mat] = Layout::vertical([Constraint::Min(8), Constraint::Length(app.topo.gpus().len() as u16 + 3)]).areas(der);
    let filas = app.filas();
    let items: Vec<ListItem> = filas
        .iter()
        .map(|&(nivel, i)| {
            let d = &app.topo.dispositivos[i];
            let alerta = if d.degradado().is_some() { " ⚠" } else { "" };
            let txt = format!("{}{} {} {}{}", "  ".repeat(nivel), d.tipo.icono(), d.bdf, d.nombre_vendor(), alerta);
            let estilo = match d.tipo {
                Tipo::Gpu => Style::default().fg(Color::Green),
                Tipo::Puente => Style::default().fg(Color::DarkGray),
                _ => Style::default(),
            };
            ListItem::new(txt).style(if alerta.is_empty() { estilo } else { estilo.fg(Color::Yellow) })
        })
        .collect();
    let mut st = ListState::default();
    st.select((!filas.is_empty()).then_some(app.sel.min(filas.len().saturating_sub(1))));
    let titulo = if app.solo_gpus { " Topología (sólo GPUs) " } else { " Topología PCIe " };
    f.render_stateful_widget(List::new(items).block(Block::default().borders(Borders::ALL).title(titulo)).highlight_style(Style::default().add_modifier(Modifier::REVERSED)), izq, &mut st);

    let detalle: Vec<Line> = match filas.get(app.sel) {
        Some(&(_, i)) => {
            let d = &app.topo.dispositivos[i];
            let mut v = vec![
                Line::from(format!("BDF:     {}", d.bdf)),
                Line::from(format!("Vendor:  {:04x} ({})  Device: {:04x}", d.vendor, d.nombre_vendor(), d.device)),
                Line::from(format!("Clase:   {:06x} → {:?}", d.clase, d.tipo)),
                Line::from(format!("Enlace:  {}", d.actual.map(|e| e.to_string()).unwrap_or("—".into()))),
                Line::from(format!("Máximo:  {}", d.maximo.map(|e| e.to_string()).unwrap_or("—".into()))),
                Line::from(format!("NUMA: {}  Driver: {}", d.numa.map(|n| n.to_string()).unwrap_or("-".into()), d.driver.as_deref().unwrap_or("-"))),
                Line::from(format!("Root:    {}  Saltos: {}", d.raiz, app.topo.ancestros(&d.bdf).len())),
            ];
            if let Some(c) = app.topo.cuello_de_botella(&d.bdf) {
                v.push(Line::from(format!("Cuello de botella: {c:.1} GB/s")));
            }
            if let Some(w) = d.degradado() {
                v.push(Line::from(format!("⚠ Degradado: {w}")).style(Style::default().fg(Color::Yellow)));
            }
            v
        }
        None => vec![Line::from("Sin dispositivos")],
    };
    f.render_widget(Paragraph::new(detalle).wrap(Wrap { trim: true }).block(Block::default().borders(Borders::ALL).title(" Detalle ")), det);
    f.render_widget(Paragraph::new(app.topo.matriz_texto()).block(Block::default().borders(Borders::ALL).title(" GPU×GPU  j/k:mover g:filtrar q:salir ")), mat);
}
