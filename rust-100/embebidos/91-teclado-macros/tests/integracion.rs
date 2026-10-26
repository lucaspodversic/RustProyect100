//! Pruebas de integración del macropad con una matriz simulada.
//!
//! `MatrizFalsa` guarda qué teclas están "físicamente" pulsadas y qué fila
//! tiene el firmware en bajo. Las columnas leen bajo si la tecla de esa fila
//! activa está pulsada, igual que en la placa real.

use core::convert::Infallible;
use embedded_hal::digital::{ErrorType, InputPin, OutputPin};
use std::cell::RefCell;
use std::rc::Rc;
use teclado_macros::hid::{kc, mods, Reporte};
use teclado_macros::macros::MACROS;
use teclado_macros::mapa::MAPA_DEFECTO;
use teclado_macros::matriz::Matriz;
use teclado_macros::teclado::Teclado;

#[derive(Default)]
struct Estado {
    pulsadas: [[bool; 3]; 3],
    fila_activa: Option<usize>,
}
type Compartido = Rc<RefCell<Estado>>;

struct Fila(usize, Compartido);
impl ErrorType for Fila {
    type Error = Infallible;
}
impl OutputPin for Fila {
    fn set_low(&mut self) -> Result<(), Infallible> {
        self.1.borrow_mut().fila_activa = Some(self.0);
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Infallible> {
        let mut e = self.1.borrow_mut();
        if e.fila_activa == Some(self.0) {
            e.fila_activa = None;
        }
        Ok(())
    }
}
struct Col(usize, Compartido);
impl ErrorType for Col {
    type Error = Infallible;
}
impl InputPin for Col {
    fn is_high(&mut self) -> Result<bool, Infallible> {
        self.is_low().map(|b| !b)
    }
    fn is_low(&mut self) -> Result<bool, Infallible> {
        let e = self.1.borrow();
        Ok(e.fila_activa.is_some_and(|f| e.pulsadas[f][self.0]))
    }
}

fn armar() -> (Teclado<Fila, Col>, Compartido) {
    let s: Compartido = Default::default();
    let m = Matriz::new([0, 1, 2].map(|i| Fila(i, s.clone())), [0, 1, 2].map(|i| Col(i, s.clone())));
    (Teclado::new(m, MAPA_DEFECTO, &MACROS), s)
}

/// Avanza `n` ticks y junta los reportes emitidos.
fn ticks(t: &mut Teclado<Fila, Col>, n: usize) -> Vec<Reporte> {
    (0..n).filter_map(|_| t.tick()).collect()
}

#[test]
fn ctrl_c_con_antirrebote() {
    let (mut t, s) = armar();
    s.borrow_mut().pulsadas[0][0] = true;
    assert!(ticks(&mut t, 4).is_empty(), "aún no supera el antirrebote");
    let r = ticks(&mut t, 1);
    assert_eq!(r[0].bytes(), [mods::LCTRL, 0, kc::C, 0, 0, 0, 0, 0]);
    s.borrow_mut().pulsadas[0][0] = false;
    let r = ticks(&mut t, 10);
    assert_eq!(r, vec![Reporte::default()]);
}

#[test]
fn capa_fn_y_tecla_congelada() {
    let (mut t, s) = armar();
    s.borrow_mut().pulsadas[2][2] = true; // Fn
    ticks(&mut t, 6);
    assert_eq!(t.capas(), 0b11);
    s.borrow_mut().pulsadas[0][2] = true; // en capa 1 = Vol+
    let r = ticks(&mut t, 6);
    assert_eq!(r.last().unwrap().teclas[0], kc::VOL_ARRIBA);
    // Se suelta Fn primero: Vol+ sigue pulsada (acción congelada).
    s.borrow_mut().pulsadas[2][2] = false;
    let r = ticks(&mut t, 6);
    assert!(r.is_empty());
    assert_eq!(t.capas(), 1);
}

#[test]
fn macro_firma_completa() {
    let (mut t, s) = armar();
    s.borrow_mut().pulsadas[1][1] = true; // Macro 0
    let r = ticks(&mut t, 200);
    // "Saludos,\nAna": la coma no está en la tabla y se omite →
    // 11 caracteres × (pulsar + soltar) = 22 reportes.
    assert_eq!(r.len(), 22);
    assert_eq!(r[0].modificadores, mods::LSHIFT); // 'S'
    let texto: String = r
        .iter()
        .filter(|x| x.teclas[0] != 0)
        .map(|x| match x.teclas[0] {
            c @ 0x04..=0x1D => {
                let ch = (b'a' + c - 0x04) as char;
                if x.modificadores & mods::LSHIFT != 0 { ch.to_ascii_uppercase() } else { ch }
            }
            0x28 => '\n',
            _ => '?',
        })
        .collect();
    assert_eq!(texto, "Saludos\nAna");
}

#[test]
fn varias_teclas_a_la_vez() {
    let (mut t, s) = armar();
    {
        let mut e = s.borrow_mut();
        e.pulsadas[2][0] = true; // Esc
        e.pulsadas[2][1] = true; // Enter
        e.pulsadas[1][0] = true; // Ctrl+S
    }
    let r = ticks(&mut t, 6);
    let ult = r.last().unwrap();
    assert_eq!(ult.modificadores, mods::LCTRL);
    let mut teclas: Vec<u8> = ult.teclas.iter().copied().filter(|&c| c != 0).collect();
    teclas.sort();
    assert_eq!(teclas, vec![kc::S, kc::ENTER, kc::ESC]);
}
