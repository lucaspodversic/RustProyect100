//! Planificador de riego.
//!
//! [`Riego::tick`] se llama periódicamente (p. ej. cada segundo) con la
//! hora del día y el tiempo transcurrido. En cada tick:
//!
//! 1. Lee lluvia y nivel de tanque (entradas digitales).
//! 2. Mide cada zona que no esté regando ni en remojo y actualiza su estado.
//! 3. Si la zona activa debe cortar (humedad alta, tiempo máximo o un
//!    bloqueo global), cierra válvula y pasa a remojo.
//! 4. Si no hay zona activa y se puede regar, elige la **pendiente más seca**.
//! 5. Aplica salidas: válvulas y bomba. **Orden seguro**: al encender, primero
//!    se abre la válvula y luego la bomba; al apagar, primero la bomba (evita
//!    golpe de ariete y que la bomba trabaje contra un circuito cerrado).

use embedded_hal::digital::{InputPin, OutputPin};

use crate::sensor::{mediana5, Adc, Humedad};
use crate::zona::{EstadoZona, Zona};

/// Motivo por el que el sistema no puede regar ahora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bloqueo {
    /// El sensor de lluvia está mojado.
    Lluvia,
    /// El flotante indica tanque vacío.
    TanqueVacio,
    /// Fuera de la franja horaria permitida.
    FueraDeHorario,
}

/// Franja horaria permitida (minutos del día). Si `desde > hasta`, la franja
/// cruza la medianoche (p. ej. 20:00 → 09:00).
#[derive(Debug, Clone, Copy)]
pub struct Franja {
    /// Inicio (incluido).
    pub desde: u16,
    /// Fin (excluido).
    pub hasta: u16,
}

impl Franja {
    /// ¿El minuto está dentro?
    pub fn contiene(&self, m: u16) -> bool {
        if self.desde <= self.hasta {
            m >= self.desde && m < self.hasta
        } else {
            m >= self.desde || m < self.hasta
        }
    }
}

/// Hardware de una zona: sensor y válvula.
pub struct HwZona<A, V> {
    /// ADC del sensor.
    pub adc: A,
    /// Electroválvula.
    pub valvula: V,
}

/// Controlador de `N` zonas.
pub struct Riego<A, V, B, LL, TQ, const N: usize> {
    /// Estado lógico de las zonas.
    pub zonas: [Zona; N],
    hw: [HwZona<A, V>; N],
    bomba: B,
    lluvia: LL,
    tanque: TQ,
    /// Franja permitida.
    pub franja: Franja,
    activa: Option<usize>,
}

impl<A, V, B, LL, TQ, const N: usize> Riego<A, V, B, LL, TQ, N>
where
    A: Adc,
    V: OutputPin,
    B: OutputPin,
    LL: InputPin,
    TQ: InputPin,
{
    /// Crea el sistema. Franja por defecto: 19:00 → 10:00.
    pub fn new(zonas: [Zona; N], hw: [HwZona<A, V>; N], bomba: B, lluvia: LL, tanque: TQ) -> Self {
        Self {
            zonas,
            hw,
            bomba,
            lluvia,
            tanque,
            franja: Franja { desde: 19 * 60, hasta: 10 * 60 },
            activa: None,
        }
    }

    /// Zona regando ahora.
    pub fn activa(&self) -> Option<usize> {
        self.activa
    }

    /// Acceso al hardware (tests).
    pub fn hw(&mut self) -> (&mut [HwZona<A, V>; N], &mut B) {
        (&mut self.hw, &mut self.bomba)
    }

    /// Bloqueo global actual, si lo hay. Sensor de lluvia: activo en bajo
    /// (el módulo YL‑83 baja la salida cuando se moja). Flotante del tanque:
    /// alto = hay agua. Si una lectura falla, se asume la condición insegura.
    pub fn bloqueo(&mut self, minuto: u16) -> Option<Bloqueo> {
        if self.lluvia.is_low().unwrap_or(true) {
            Some(Bloqueo::Lluvia)
        } else if !self.tanque.is_high().unwrap_or(false) {
            Some(Bloqueo::TanqueVacio)
        } else if !self.franja.contiene(minuto) {
            Some(Bloqueo::FueraDeHorario)
        } else {
            None
        }
    }

    /// Mide la zona `i` y devuelve la humedad (o `None` si falla).
    fn medir(&mut self, i: usize) -> Option<u8> {
        let h = match mediana5(&mut self.hw[i].adc) {
            Ok(raw) => self.zonas[i].cfg.calibracion.a_porcentaje(raw),
            Err(()) => Humedad::Falla,
        };
        match h {
            Humedad::Pct(p) => {
                self.zonas[i].humedad = Some(p);
                Some(p)
            }
            Humedad::Falla => {
                self.zonas[i].humedad = None;
                None
            }
        }
    }

    /// Un paso del controlador.
    pub fn tick(&mut self, minuto: u16, dt_ms: u32) -> Option<Bloqueo> {
        let bloqueo = self.bloqueo(minuto);

        // --- 1) Actualizar cada zona ---------------------------------------
        for i in 0..N {
            match self.zonas[i].estado {
                EstadoZona::Regando(ms) => {
                    let ms = ms + dt_ms;
                    let z = self.zonas[i];
                    self.zonas[i].ml_hoy += z.cfg.caudal_ml_s * dt_ms / 1000;
                    let humedad = self.medir(i);
                    let cortar = bloqueo.is_some()
                        || ms >= z.cfg.max_riego_ms
                        || humedad.map_or(true, |h| h >= z.cfg.umbral_alto);
                    self.zonas[i].estado = if cortar {
                        self.activa = None;
                        EstadoZona::Remojo(z.cfg.remojo_ms)
                    } else {
                        EstadoZona::Regando(ms)
                    };
                }
                EstadoZona::Remojo(resta) => {
                    self.zonas[i].estado = if resta > dt_ms {
                        EstadoZona::Remojo(resta - dt_ms)
                    } else {
                        EstadoZona::Ok // vuelve a evaluarse en el próximo tick
                    };
                }
                _ => {
                    let z = self.zonas[i];
                    self.zonas[i].estado = match self.medir(i) {
                        None => EstadoZona::FallaSensor,
                        Some(h) if h < z.cfg.umbral_bajo => EstadoZona::Pendiente,
                        // Si ya estaba pendiente, sigue pendiente hasta superar
                        // el umbral alto (histéresis también en la espera).
                        Some(h) if z.estado == EstadoZona::Pendiente && h < z.cfg.umbral_alto => {
                            EstadoZona::Pendiente
                        }
                        Some(_) => EstadoZona::Ok,
                    };
                }
            }
        }

        // --- 2) Elegir zona si no hay ninguna activa ------------------------
        if self.activa.is_none() && bloqueo.is_none() {
            let elegida = (0..N)
                .filter(|&i| self.zonas[i].estado == EstadoZona::Pendiente)
                .min_by_key(|&i| self.zonas[i].humedad.unwrap_or(u8::MAX));
            if let Some(i) = elegida {
                self.zonas[i].estado = EstadoZona::Regando(0);
                self.zonas[i].ciclos_hoy += 1;
                self.activa = Some(i);
            }
        }

        // --- 3) Salidas en orden seguro -------------------------------------
        match self.activa {
            Some(a) => {
                for (i, hw) in self.hw.iter_mut().enumerate() {
                    if i != a {
                        let _ = hw.valvula.set_low();
                    }
                }
                let _ = self.hw[a].valvula.set_high();
                let _ = self.bomba.set_high();
            }
            None => {
                let _ = self.bomba.set_low();
                for hw in self.hw.iter_mut() {
                    let _ = hw.valvula.set_low();
                }
            }
        }
        bloqueo
    }

    /// Reinicia contadores diarios de todas las zonas.
    pub fn nuevo_dia(&mut self) {
        for z in self.zonas.iter_mut() {
            z.nuevo_dia();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zona::ConfigZona;
    use core::convert::Infallible;
    use embedded_hal::digital::ErrorType;
    use std::cell::Cell;
    use std::rc::Rc;

    /// ADC cuya lectura cambia el test (compartida por Rc).
    #[derive(Clone)]
    struct AdcFalso(Rc<Cell<u16>>);
    impl Adc for AdcFalso {
        fn leer(&mut self) -> Result<u16, ()> {
            Ok(self.0.get())
        }
    }

    /// Pin de salida que registra su nivel.
    #[derive(Clone, Default)]
    struct Salida(Rc<Cell<bool>>);
    impl ErrorType for Salida {
        type Error = Infallible;
    }
    impl OutputPin for Salida {
        fn set_high(&mut self) -> Result<(), Infallible> {
            self.0.set(true);
            Ok(())
        }
        fn set_low(&mut self) -> Result<(), Infallible> {
            self.0.set(false);
            Ok(())
        }
    }

    /// Entrada configurable.
    #[derive(Clone)]
    struct Entrada(Rc<Cell<bool>>);
    impl ErrorType for Entrada {
        type Error = Infallible;
    }
    impl InputPin for Entrada {
        fn is_high(&mut self) -> Result<bool, Infallible> {
            Ok(self.0.get())
        }
        fn is_low(&mut self) -> Result<bool, Infallible> {
            Ok(!self.0.get())
        }
    }

    /// Humedad % → lectura cruda con la calibración por defecto.
    fn raw(pct: u16) -> u16 {
        3000 - pct * 17
    }

    struct Banco {
        adcs: [Rc<Cell<u16>>; 2],
        valvulas: [Rc<Cell<bool>>; 2],
        bomba: Rc<Cell<bool>>,
        lluvia_seco: Rc<Cell<bool>>,
        tanque_lleno: Rc<Cell<bool>>,
    }

    type Sistema = Riego<AdcFalso, Salida, Salida, Entrada, Entrada, 2>;

    fn armar() -> (Sistema, Banco) {
        let b = Banco {
            adcs: [Rc::new(Cell::new(raw(60))), Rc::new(Cell::new(raw(60)))],
            valvulas: Default::default(),
            bomba: Default::default(),
            lluvia_seco: Rc::new(Cell::new(true)),
            tanque_lleno: Rc::new(Cell::new(true)),
        };
        let cfg = ConfigZona { max_riego_ms: 60_000, remojo_ms: 10_000, ..Default::default() };
        let hw = [0, 1].map(|i| HwZona { adc: AdcFalso(b.adcs[i].clone()), valvula: Salida(b.valvulas[i].clone()) });
        let r = Riego::new(
            [Zona::new(cfg), Zona::new(cfg)],
            hw,
            Salida(b.bomba.clone()),
            Entrada(b.lluvia_seco.clone()),
            Entrada(b.tanque_lleno.clone()),
        );
        (r, b)
    }

    const NOCHE: u16 = 21 * 60;

    #[test]
    fn riega_la_mas_seca_y_corta_por_humedad() {
        let (mut r, b) = armar();
        b.adcs[0].set(raw(20));
        b.adcs[1].set(raw(10));
        r.tick(NOCHE, 1000);
        assert_eq!(r.activa(), Some(1), "elige la más seca");
        assert!(b.valvulas[1].get() && !b.valvulas[0].get() && b.bomba.get());
        // Entre umbrales (35 %) sigue regando.
        b.adcs[1].set(raw(35));
        r.tick(NOCHE, 1000);
        assert_eq!(r.activa(), Some(1));
        // Supera el alto (50 %): corta y pasa a remojo; entra la zona 0.
        b.adcs[1].set(raw(50));
        r.tick(NOCHE, 1000);
        assert_eq!(r.zonas[1].estado, EstadoZona::Remojo(10_000));
        assert_eq!(r.activa(), Some(0));
        assert!(b.valvulas[0].get() && !b.valvulas[1].get());
        assert_eq!(r.zonas[1].ml_hoy, 50 * 2);
    }

    #[test]
    fn tiempo_maximo_y_remojo() {
        let (mut r, b) = armar();
        b.adcs[0].set(raw(5));
        r.tick(NOCHE, 1000);
        for _ in 0..60 {
            r.tick(NOCHE, 1000);
        }
        assert!(matches!(r.zonas[0].estado, EstadoZona::Remojo(_)));
        assert!(!b.bomba.get());
        // Durante el remojo no se vuelve a regar aunque siga seco.
        for _ in 0..9 {
            r.tick(NOCHE, 1000);
            assert_eq!(r.activa(), None);
        }
        r.tick(NOCHE, 1000); // termina remojo → Ok
        r.tick(NOCHE, 1000); // re‑evalúa → pendiente → riega
        assert_eq!(r.activa(), Some(0));
        assert_eq!(r.zonas[0].ciclos_hoy, 2);
    }

    #[test]
    fn bloqueos_lluvia_tanque_horario() {
        let (mut r, b) = armar();
        b.adcs[0].set(raw(5));
        assert_eq!(r.tick(13 * 60, 1000), Some(Bloqueo::FueraDeHorario));
        assert_eq!(r.activa(), None);
        b.lluvia_seco.set(false);
        assert_eq!(r.tick(NOCHE, 1000), Some(Bloqueo::Lluvia));
        b.lluvia_seco.set(true);
        b.tanque_lleno.set(false);
        assert_eq!(r.tick(NOCHE, 1000), Some(Bloqueo::TanqueVacio));
        b.tanque_lleno.set(true);
        assert_eq!(r.tick(NOCHE, 1000), None);
        assert_eq!(r.activa(), Some(0));
        // Empieza a llover durante el riego: corta inmediatamente.
        b.lluvia_seco.set(false);
        r.tick(NOCHE, 1000);
        assert_eq!(r.activa(), None);
        assert!(!b.bomba.get() && !b.valvulas[0].get());
    }

    #[test]
    fn sensor_roto_no_riega() {
        let (mut r, b) = armar();
        b.adcs[0].set(4095);
        r.tick(NOCHE, 1000);
        assert_eq!(r.zonas[0].estado, EstadoZona::FallaSensor);
        assert_eq!(r.activa(), None);
        b.adcs[0].set(raw(50));
        r.tick(NOCHE, 1000);
        assert_eq!(r.zonas[0].estado, EstadoZona::Ok);
    }

    #[test]
    fn franja_que_cruza_medianoche() {
        let f = Franja { desde: 20 * 60, hasta: 9 * 60 };
        assert!(f.contiene(23 * 60) && f.contiene(60) && !f.contiene(12 * 60));
        let f2 = Franja { desde: 60, hasta: 120 };
        assert!(f2.contiene(90) && !f2.contiene(120));
    }

    #[test]
    fn bomba_con_pin_mock() {
        // Verifica con embedded-hal-mock que sin necesidad de riego la bomba
        // se mantiene en bajo en cada tick.
        use embedded_hal_mock::eh1::digital::{Mock, State, Transaction};
        let (r, b) = armar();
        let Riego { zonas, hw, lluvia, tanque, .. } = r;
        let mut bomba = Mock::new(&[Transaction::set(State::Low), Transaction::set(State::Low)]);
        let mut r2 = Riego::new(zonas, hw, bomba.clone(), lluvia, tanque);
        r2.tick(NOCHE, 1000);
        r2.tick(NOCHE, 1000);
        bomba.done();
        drop(b);
    }
}
