//! Hora del día con aritmética módulo 24 h.

/// Segundos que tiene un día.
pub const SEGUNDOS_DIA: u32 = 86_400;

/// Hora del día representada como segundos desde la medianoche.
/// Guardar un único entero simplifica sumar/restar y comparar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Hora(u32);

impl Hora {
    /// Construye desde horas/minutos/segundos (se normaliza módulo 24 h).
    pub const fn hms(h: u32, m: u32, s: u32) -> Self {
        Hora((h * 3600 + m * 60 + s) % SEGUNDOS_DIA)
    }
    /// Horas 0..24.
    pub const fn h(self) -> u32 {
        self.0 / 3600
    }
    /// Minutos 0..60.
    pub const fn m(self) -> u32 {
        (self.0 / 60) % 60
    }
    /// Segundos 0..60.
    pub const fn s(self) -> u32 {
        self.0 % 60
    }
    /// Segundos desde medianoche.
    pub const fn total(self) -> u32 {
        self.0
    }
    /// Suma (o resta, con negativos) segundos, dando la vuelta al día.
    pub fn sumar(self, segundos: i64) -> Self {
        let d = SEGUNDOS_DIA as i64;
        Hora(((self.0 as i64 + segundos).rem_euclid(d)) as u32)
    }
    /// Suma horas manteniendo minutos (para el modo ajuste).
    pub fn sumar_horas(self, n: i32) -> Self {
        self.sumar(n as i64 * 3600)
    }
    /// Suma minutos **sin** tocar la hora (al pasar de 59 vuelve a 00, como
    /// en cualquier reloj de botones).
    pub fn sumar_minutos_ciclico(self, n: i32) -> Self {
        let m = (self.m() as i32 + n).rem_euclid(60) as u32;
        Hora::hms(self.h(), m, self.s())
    }
    /// ¿Coinciden horas y minutos? (la alarma ignora los segundos).
    pub fn mismo_minuto(self, otra: Hora) -> bool {
        self.0 / 60 == otra.0 / 60
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn componentes_y_vuelta_del_dia() {
        let t = Hora::hms(23, 59, 30);
        assert_eq!((t.h(), t.m(), t.s()), (23, 59, 30));
        let t2 = t.sumar(45);
        assert_eq!((t2.h(), t2.m(), t2.s()), (0, 0, 15));
        assert_eq!(Hora::hms(0, 0, 0).sumar(-1), Hora::hms(23, 59, 59));
    }

    #[test]
    fn ajustes_ciclicos() {
        let t = Hora::hms(10, 59, 0);
        assert_eq!(t.sumar_minutos_ciclico(1), Hora::hms(10, 0, 0));
        assert_eq!(Hora::hms(0, 5, 0).sumar_horas(-1), Hora::hms(23, 5, 0));
        assert!(Hora::hms(7, 30, 0).mismo_minuto(Hora::hms(7, 30, 59)));
    }
}
