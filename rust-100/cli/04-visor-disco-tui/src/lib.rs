//! # Visor de uso de disco — lógica + vista
//!
//! Se divide en tres capas, para poder testear sin terminal real:
//! - **Modelo** ([`Nodo`], [`escanear`]): árbol de directorios con tamaño acumulado.
//! - **Estado** ([`App`]): directorio actual, selección, navegación (entrar/subir).
//! - **Vista** ([`dibujar`]): render con `ratatui`. En los tests se usa
//!   `TestBackend` (un buffer en memoria), así no hace falta un display.

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph};
use ratatui::Frame;
use std::fs;
use std::path::{Path, PathBuf};

/// Nodo del árbol: archivo o directorio con su tamaño total en bytes.
#[derive(Debug, Clone)]
pub struct Nodo {
    pub nombre: String,
    pub ruta: PathBuf,
    /// Para directorios: suma recursiva de todos sus archivos.
    pub tamano: u64,
    pub es_dir: bool,
    /// Hijos ordenados de mayor a menor tamaño (vacío para archivos).
    pub hijos: Vec<Nodo>,
}

/// Escanea recursivamente `ruta`. No sigue symlinks (evita ciclos) y
/// tolera errores de permisos (cuentan como tamaño 0).
pub fn escanear(ruta: &Path) -> Nodo {
    let nombre = ruta.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| ruta.display().to_string());
    // `symlink_metadata` NO sigue el enlace: un symlink se trata como archivo chico.
    let meta = fs::symlink_metadata(ruta);
    let es_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
    if !es_dir {
        let tamano = meta.map(|m| m.len()).unwrap_or(0);
        return Nodo { nombre, ruta: ruta.to_path_buf(), tamano, es_dir: false, hijos: vec![] };
    }
    let mut hijos: Vec<Nodo> = fs::read_dir(ruta)
        .map(|rd| rd.filter_map(Result::ok).map(|e| escanear(&e.path())).collect())
        .unwrap_or_default();
    // Mayor primero; a igual tamaño, por nombre (orden estable para tests).
    hijos.sort_by(|a, b| b.tamano.cmp(&a.tamano).then(a.nombre.cmp(&b.nombre)));
    let tamano = hijos.iter().map(|h| h.tamano).sum();
    Nodo { nombre, ruta: ruta.to_path_buf(), tamano, es_dir: true, hijos }
}

/// Formatea bytes en unidades binarias legibles (KiB, MiB, ...).
pub fn humano(b: u64) -> String {
    const U: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{b} B") } else { format!("{v:.1} {}", U[i]) }
}

/// Estado de la aplicación TUI.
pub struct App {
    pub raiz: Nodo,
    /// Camino de índices desde la raíz hasta el directorio que se está viendo.
    pub camino: Vec<usize>,
    /// Estado de selección de la lista (ratatui lo usa para resaltar).
    pub lista: ListState,
    pub salir: bool,
}

impl App {
    pub fn new(raiz: Nodo) -> Self {
        let mut lista = ListState::default();
        lista.select(if raiz.hijos.is_empty() { None } else { Some(0) });
        App { raiz, camino: vec![], lista, salir: false }
    }

    /// Nodo (directorio) que se está mostrando.
    pub fn actual(&self) -> &Nodo {
        let mut n = &self.raiz;
        for &i in &self.camino {
            n = &n.hijos[i];
        }
        n
    }

    pub fn abajo(&mut self) {
        let len = self.actual().hijos.len();
        if len == 0 {
            return;
        }
        let i = self.lista.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
        self.lista.select(Some(i));
    }

    pub fn arriba(&mut self) {
        let i = self.lista.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
        if !self.actual().hijos.is_empty() {
            self.lista.select(Some(i));
        }
    }

    /// Entra al directorio seleccionado (si lo es y tiene contenido).
    pub fn entrar(&mut self) {
        if let Some(i) = self.lista.selected() {
            let h = &self.actual().hijos[i];
            if h.es_dir && !h.hijos.is_empty() {
                self.camino.push(i);
                self.lista.select(Some(0));
            }
        }
    }

    /// Sube al padre, re-seleccionando el directorio del que venimos.
    pub fn subir(&mut self) {
        if let Some(i) = self.camino.pop() {
            self.lista.select(Some(i));
        }
    }

    /// Procesa una tecla (abstraída como `char` para testear sin crossterm).
    /// `j/k` o flechas mapeadas por el binario, `l`/Enter entra, `h`/Backspace sube, `q` sale.
    pub fn tecla(&mut self, c: char) {
        match c {
            'j' => self.abajo(),
            'k' => self.arriba(),
            'l' | '\n' => self.entrar(),
            'h' | '\x08' => self.subir(),
            'q' => self.salir = true,
            _ => {}
        }
    }
}

/// Dibuja la interfaz: cabecera, lista con barras y pie de ayuda.
pub fn dibujar(f: &mut Frame, app: &mut App) {
    let zonas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Length(3), Constraint::Length(1)])
        .split(f.area());

    let actual = app.actual().clone();
    let total = actual.tamano.max(1);

    // Cabecera con ruta y total.
    let cab = Paragraph::new(format!("{}  —  {}", actual.ruta.display(), humano(actual.tamano)))
        .block(Block::default().borders(Borders::ALL).title(" Uso de disco "));
    f.render_widget(cab, zonas[0]);

    // Lista de hijos con barra proporcional al tamaño.
    let items: Vec<ListItem> = actual
        .hijos
        .iter()
        .map(|h| {
            let pct = h.tamano as f64 / total as f64;
            let barra = "█".repeat((pct * 20.0).round() as usize);
            let icono = if h.es_dir { "📁" } else { "  " };
            ListItem::new(Line::from(format!("{:>10} {:5.1}% {:<20} {icono} {}", humano(h.tamano), pct * 100.0, barra, h.nombre)))
        })
        .collect();
    let lista = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Contenido (mayor primero) "))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(lista, zonas[1], &mut app.lista);

    // Gauge con el porcentaje del elemento seleccionado.
    let sel = app.lista.selected().and_then(|i| actual.hijos.get(i));
    let ratio = sel.map(|h| h.tamano as f64 / total as f64).unwrap_or(0.0);
    let g = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Selección "))
        .gauge_style(Style::default().fg(Color::Green))
        .ratio(ratio.clamp(0.0, 1.0))
        .label(sel.map(|h| format!("{} ({})", h.nombre, humano(h.tamano))).unwrap_or_default());
    f.render_widget(g, zonas[2]);

    f.render_widget(Paragraph::new("↑/k ↓/j mover · Enter/l entrar · Backspace/h subir · q salir"), zonas[3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn arbol() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("grande/sub")).unwrap();
        fs::write(d.path().join("grande/sub/x.bin"), vec![0u8; 5000]).unwrap();
        fs::write(d.path().join("grande/y.bin"), vec![0u8; 1000]).unwrap();
        fs::write(d.path().join("chico.txt"), vec![0u8; 10]).unwrap();
        d
    }

    #[test]
    fn suma_y_ordena() {
        let d = arbol();
        let n = escanear(d.path());
        assert_eq!(n.tamano, 6010);
        assert_eq!(n.hijos[0].nombre, "grande");
        assert_eq!(n.hijos[0].hijos[0].nombre, "sub");
    }

    #[test]
    fn formato_humano() {
        assert_eq!(humano(512), "512 B");
        assert_eq!(humano(2048), "2.0 KiB");
        assert_eq!(humano(5 * 1024 * 1024), "5.0 MiB");
    }

    #[test]
    fn navegacion() {
        let d = arbol();
        let mut app = App::new(escanear(d.path()));
        app.tecla('l'); // entra a "grande"
        assert_eq!(app.actual().nombre, "grande");
        app.tecla('j');
        assert_eq!(app.lista.selected(), Some(1));
        app.tecla('j'); // no pasa del final
        assert_eq!(app.lista.selected(), Some(1));
        app.tecla('h'); // vuelve y re-selecciona "grande"
        assert_eq!(app.camino.len(), 0);
        assert_eq!(app.lista.selected(), Some(0));
        app.tecla('q');
        assert!(app.salir);
    }

    #[test]
    fn no_entra_a_archivos() {
        let d = arbol();
        let mut app = App::new(escanear(d.path()));
        app.tecla('j'); // chico.txt
        app.tecla('l');
        assert!(app.camino.is_empty());
    }

    /// Render "headless" en un buffer de 80x20: comprueba que aparezcan los textos.
    #[test]
    fn render_en_test_backend() {
        let d = arbol();
        let mut app = App::new(escanear(d.path()));
        let mut t = Terminal::new(TestBackend::new(90, 20)).unwrap();
        t.draw(|f| dibujar(f, &mut app)).unwrap();
        let buf = t.backend().buffer();
        let texto: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(texto.contains("Uso de disco"));
        assert!(texto.contains("grande"));
        assert!(texto.contains("chico.txt"));
    }
}
