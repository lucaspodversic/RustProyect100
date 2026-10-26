//! # Explorador de bloques TUI (proyecto 69)
//!
//! - [`datos`]: trait `FuenteCadena` + cadena simulada determinista.
//! - [`app`]: estado y navegación (sin terminal).
//! - [`ui`]: dibujo con ratatui (testeable con `TestBackend`).

pub mod app;
pub mod datos;
pub mod ui;

/// Renderiza el estado actual a texto plano (útil para tests y `--snapshot`).
pub fn renderizar_texto<F: datos::FuenteCadena>(app: &app::App<F>, ancho: u16, alto: u16) -> String {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let mut term = Terminal::new(TestBackend::new(ancho, alto)).expect("TestBackend nunca falla");
    term.draw(|f| ui::dibujar(f, app)).expect("dibujo");
    let buf = term.backend().buffer().clone();
    let mut s = String::new();
    for y in 0..alto {
        for x in 0..ancho {
            s.push_str(buf[(x, y)].symbol());
        }
        s.push('\n');
    }
    s
}
