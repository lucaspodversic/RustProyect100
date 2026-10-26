//! Estado de la UI como **reductor puro** (estilo Elm/Redux).
//!
//! Toda la lógica de la interfaz vive acá, sin DOM ni red → se testea en
//! nativo con `cargo test`, aunque no tengamos navegador ni target wasm32.

use compartido::{validar_texto, Nota};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct EstadoUi {
    pub notas: Vec<Nota>,
    pub borrador: String,
    pub error: Option<String>,
    pub filtro: Filtro,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Filtro {
    #[default]
    Todas,
    Pendientes,
    Hechas,
}

#[derive(Debug, Clone)]
pub enum Accion {
    Escribir(String),
    /// El usuario envía el formulario; si es válido, se espera `Creada`.
    Enviar,
    /// Respuesta del servidor con la nota creada.
    Creada(Nota),
    /// Respuesta del servidor tras alternar.
    Actualizada(Nota),
    Borrada(u32),
    Filtrar(Filtro),
    ErrorRed(String),
}

impl EstadoUi {
    /// Aplica una acción y devuelve el nuevo estado (sin efectos secundarios).
    pub fn reducir(mut self, a: Accion) -> Self {
        match a {
            Accion::Escribir(t) => {
                self.borrador = t;
                self.error = None;
            }
            Accion::Enviar => {
                if let Err(e) = validar_texto(&self.borrador) {
                    self.error = Some(e);
                }
            }
            Accion::Creada(n) => {
                self.notas.push(n);
                self.borrador.clear();
                self.error = None;
            }
            Accion::Actualizada(n) => {
                if let Some(x) = self.notas.iter_mut().find(|x| x.id == n.id) {
                    *x = n;
                }
            }
            Accion::Borrada(id) => self.notas.retain(|n| n.id != id),
            Accion::Filtrar(f) => self.filtro = f,
            Accion::ErrorRed(e) => self.error = Some(format!("error de red: {e}")),
        }
        self
    }

    pub fn visibles(&self) -> Vec<&Nota> {
        self.notas
            .iter()
            .filter(|n| match self.filtro {
                Filtro::Todas => true,
                Filtro::Pendientes => !n.hecha,
                Filtro::Hechas => n.hecha,
            })
            .collect()
    }

    pub fn pendientes(&self) -> usize {
        self.notas.iter().filter(|n| !n.hecha).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nota(id: u32, hecha: bool) -> Nota {
        Nota { id, texto: format!("n{id}"), hecha }
    }

    #[test]
    fn flujo_de_acciones() {
        let e = EstadoUi::default().reducir(Accion::Escribir("  ".into())).reducir(Accion::Enviar);
        assert!(e.error.is_some());
        let e = e.reducir(Accion::Escribir("comprar pan".into()));
        assert!(e.error.is_none());
        let e = e.reducir(Accion::Creada(nota(1, false))).reducir(Accion::Creada(nota(2, false)));
        assert_eq!(e.borrador, "");
        let e = e.reducir(Accion::Actualizada(nota(1, true)));
        assert_eq!(e.pendientes(), 1);
        let e = e.reducir(Accion::Filtrar(Filtro::Hechas));
        assert_eq!(e.visibles().iter().map(|n| n.id).collect::<Vec<_>>(), vec![1]);
        let e = e.reducir(Accion::Borrada(1));
        assert!(e.visibles().is_empty());
    }
}
