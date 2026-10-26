//! Renderizado con ratatui. Recibe `&App` y dibuja en un `Frame`; como no
//! hace I/O, en los tests se usa `TestBackend` y se inspecciona el buffer.

use crate::app::{App, Vista};
use crate::datos::FuenteCadena;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Row, Table};
use ratatui::Frame;

/// Abrevia un hash a `abcd…wxyz`.
pub fn corto(h: &str) -> String {
    if h.len() <= 12 { h.to_string() } else { format!("{}…{}", &h[..6], &h[h.len() - 4..]) }
}

pub fn dibujar<F: FuenteCadena>(f: &mut Frame, app: &App<F>) {
    let zonas = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(3), Constraint::Min(5), Constraint::Length(3)]).split(f.area());
    let cab = Paragraph::new(Line::from(vec![
        Span::styled(" ⛓ Explorador ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(format!("  altura: {}  ", app.fuente.altura())),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(cab, zonas[0]);

    match &app.vista {
        Vista::Bloques => lista_bloques(f, app, zonas[1]),
        Vista::DetalleBloque { altura, tx_sel } => detalle_bloque(f, app, *altura, *tx_sel, zonas[1]),
        Vista::DetalleTx { altura, indice } => detalle_tx(f, app, *altura, *indice, zonas[1]),
        Vista::Direccion(d) => direccion(f, app, d, zonas[1]),
    }

    let pie = match &app.busqueda {
        Some(q) => format!("Buscar (altura, hash o @dirección): {q}▏"),
        None => app.estado.clone(),
    };
    f.render_widget(Paragraph::new(pie).block(Block::default().borders(Borders::ALL)), zonas[2]);
}

fn lista_bloques<F: FuenteCadena>(f: &mut Frame, app: &App<F>, area: Rect) {
    let alto = app.fuente.altura();
    let items: Vec<ListItem> = (0..=alto)
        .rev()
        .filter_map(|h| app.fuente.bloque(h))
        .map(|b| ListItem::new(format!("#{:<6} {}  txs:{:<2} comisiones:{:<4} {}", b.altura, corto(&b.hash), b.txs.len(), b.total_comisiones(), b.productor)))
        .collect();
    let mut estado = ListState::default();
    estado.select(Some(app.seleccion));
    let lista = List::new(items).block(Block::default().title(" Bloques ").borders(Borders::ALL)).highlight_style(Style::default().add_modifier(Modifier::REVERSED)).highlight_symbol("▶ ");
    f.render_stateful_widget(lista, area, &mut estado);
}

fn detalle_bloque<F: FuenteCadena>(f: &mut Frame, app: &App<F>, altura: u64, sel: usize, area: Rect) {
    let Some(b) = app.fuente.bloque(altura) else { return };
    let partes = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(7), Constraint::Min(3)]).split(area);
    let info = vec![
        Line::from(format!("Altura:    {}", b.altura)),
        Line::from(format!("Hash:      {}", b.hash)),
        Line::from(format!("Previo:    {}", b.hash_previo)),
        Line::from(format!("Timestamp: {}   Productor: {}", b.timestamp, b.productor)),
        Line::from(format!("Txs: {}   Volumen: {}   Comisiones: {}", b.txs.len(), b.volumen(), b.total_comisiones())),
    ];
    f.render_widget(Paragraph::new(info).block(Block::default().title(format!(" Bloque #{altura}  (h/l: anterior/siguiente) ")).borders(Borders::ALL)), partes[0]);
    let filas: Vec<Row> = b
        .txs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let r = Row::new(vec![corto(&t.hash), t.de.clone(), t.para.clone(), t.monto.to_string(), t.comision.to_string()]);
            if i == sel { r.style(Style::default().add_modifier(Modifier::REVERSED)) } else { r }
        })
        .collect();
    let tabla = Table::new(filas, [Constraint::Length(14), Constraint::Length(8), Constraint::Length(8), Constraint::Length(8), Constraint::Length(8)])
        .header(Row::new(vec!["hash", "de", "para", "monto", "fee"]).style(Style::default().add_modifier(Modifier::BOLD)))
        .block(Block::default().title(" Transacciones ").borders(Borders::ALL));
    f.render_widget(tabla, partes[1]);
}

fn detalle_tx<F: FuenteCadena>(f: &mut Frame, app: &App<F>, altura: u64, i: usize, area: Rect) {
    let Some(t) = app.fuente.bloque(altura).and_then(|b| b.txs.get(i)) else { return };
    let txt = vec![
        Line::from(format!("Hash:     {}", t.hash)),
        Line::from(format!("Bloque:   #{altura} (posición {i})")),
        Line::from(format!("De:       {}", t.de)),
        Line::from(format!("Para:     {}", t.para)),
        Line::from(format!("Monto:    {}", t.monto)),
        Line::from(format!("Comisión: {}", t.comision)),
        Line::from(""),
        Line::from("a: ver historial del emisor · Esc: volver"),
    ];
    f.render_widget(Paragraph::new(txt).block(Block::default().title(" Transacción ").borders(Borders::ALL)), area);
}

fn direccion<F: FuenteCadena>(f: &mut Frame, app: &App<F>, dir: &str, area: Rect) {
    let txs = app.fuente.txs_de_direccion(dir);
    let saldo: i128 = txs.iter().map(|(_, t)| if t.para == dir { t.monto as i128 } else { -((t.monto + t.comision) as i128) }).sum();
    let items: Vec<ListItem> = txs.iter().map(|(h, t)| ListItem::new(format!("#{h:<6} {} {} → {} {}", corto(&t.hash), t.de, t.para, t.monto))).collect();
    f.render_widget(List::new(items).block(Block::default().title(format!(" Dirección {dir} · {} txs · saldo neto {saldo} ", txs.len())).borders(Borders::ALL)), area);
}
