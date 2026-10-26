//! Interfaz con ratatui: lista, barra de progreso y estado.
//! Se dibuja sobre cualquier `Backend`, así los tests usan `TestBackend`.

use crate::audio::BackendAudio;
use crate::lista::formato_tiempo;
use crate::reproductor::{Estado, Repetir, Reproductor};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn dibujar<B: BackendAudio>(f: &mut Frame, r: &Reproductor<B>) {
    let zonas = Layout::vertical([Constraint::Min(3), Constraint::Length(3), Constraint::Length(3)]).split(f.area());
    let actual = r.orden.get(r.cursor).copied();
    let items: Vec<ListItem> = r
        .pistas
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let marca = if Some(i) == actual { "♪ " } else { "  " };
            let artista = if p.artista.is_empty() { String::new() } else { format!(" — {}", p.artista) };
            ListItem::new(format!("{marca}{}{artista}", p.titulo))
        })
        .collect();
    let mut st = ListState::default();
    st.select(actual);
    f.render_stateful_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title(" Lista ")).highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        zonas[0],
        &mut st,
    );
    let (pos, dur) = (r.posicion(), r.duracion_actual.max(1));
    let icono = match r.estado {
        Estado::Reproduciendo => "▶",
        Estado::Pausado => "⏸",
        Estado::Detenido => "■",
    };
    f.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(format!(" {icono} {} ", r.pista_actual().map(|p| p.titulo.as_str()).unwrap_or("—"))))
            .ratio((pos as f64 / dur as f64).clamp(0.0, 1.0))
            .label(format!("{} / {}", formato_tiempo(pos), formato_tiempo(r.duracion_actual))),
        zonas[1],
    );
    let vol = if r.mute { "mute".to_string() } else { format!("{}%", r.volumen) };
    let rep = match r.repetir {
        Repetir::No => "no",
        Repetir::Todo => "todo",
        Repetir::Una => "una",
    };
    let alea = if r.aleatorio { "sí" } else { "no" };
    f.render_widget(
        Paragraph::new(format!("Vol {vol} · Repetir {rep} · Aleatorio {alea} · {}", r.mensaje))
            .block(Block::default().borders(Borders::ALL).title(" espacio:play n/b:sig/ant h/l:seek +/-:vol m:mute z:aleat r:repetir q:salir ")),
        zonas[2],
    );
}
