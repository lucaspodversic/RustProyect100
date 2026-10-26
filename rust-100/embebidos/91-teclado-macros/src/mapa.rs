//! Mapa de teclas (keymap) con capas.

use crate::hid::{kc, mods};

/// Lo que hace una tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accion {
    /// No hace nada.
    Nada,
    /// Usa la acción de la capa inferior.
    Transparente,
    /// Pulsa un código HID con modificadores (p. ej. Ctrl+C).
    Tecla(u8, u8),
    /// Activa la capa `n` mientras se mantiene pulsada (como la tecla Fn).
    CapaMomentanea(u8),
    /// Alterna la capa `n` (queda activa hasta pulsar otra vez).
    CapaAlternar(u8),
    /// Ejecuta la macro con ese índice.
    Macro(u8),
}

/// Número de capas.
pub const CAPAS: usize = 2;
/// Filas del macropad.
pub const FILAS: usize = 3;
/// Columnas del macropad.
pub const COLUMNAS: usize = 3;

/// Tipo del mapa.
pub type Mapa = [[[Accion; COLUMNAS]; FILAS]; CAPAS];

use Accion::*;

/// Mapa por defecto:
///
/// ```text
/// Capa 0 (edición)            Capa 1 (multimedia, con Fn mantenida)
/// ┌────────┬────────┬───────┐ ┌────────┬────────┬───────┐
/// │ Ctrl+C │ Ctrl+V │ Ctrl+Z│ │ Vol‑   │ Mute   │ Vol+  │
/// ├────────┼────────┼───────┤ ├────────┼────────┼───────┤
/// │ Ctrl+S │ Macro0 │ Macro1│ │ F13    │ ·      │ ·     │
/// ├────────┼────────┼───────┤ ├────────┼────────┼───────┤
/// │ Esc    │ Enter  │ Fn    │ │ ·      │ ·      │ (Fn)  │
/// └────────┴────────┴───────┘ └────────┴────────┴───────┘
/// ```
/// (`·` = transparente)
pub const MAPA_DEFECTO: Mapa = [
    [
        [Tecla(kc::C, mods::LCTRL), Tecla(kc::V, mods::LCTRL), Tecla(kc::Z, mods::LCTRL)],
        [Tecla(kc::S, mods::LCTRL), Macro(0), Macro(1)],
        [Tecla(kc::ESC, 0), Tecla(kc::ENTER, 0), CapaMomentanea(1)],
    ],
    [
        [Tecla(kc::VOL_ABAJO, 0), Tecla(kc::SILENCIO, 0), Tecla(kc::VOL_ARRIBA, 0)],
        [Tecla(kc::F13, 0), Transparente, Transparente],
        [Transparente, Transparente, Transparente],
    ],
];

/// Resuelve la acción de (fila, col) dada la máscara de capas activas: se usa
/// la capa activa más alta cuya acción no sea `Transparente`.
pub fn resolver(mapa: &Mapa, capas_activas: u8, f: usize, c: usize) -> Accion {
    for capa in (0..CAPAS).rev() {
        if capas_activas & (1 << capa) == 0 {
            continue;
        }
        let a = mapa[capa][f][c];
        if a != Transparente {
            return a;
        }
    }
    Nada
}
