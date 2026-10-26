//! Media móvil de ventana fija (buffer circular).
//!
//! El PMS5003 entrega una lectura por segundo con bastante ruido; promediar
//! las últimas N suaviza el valor que se muestra y evita alarmas espurias.

/// Media móvil sobre las últimas `N` muestras.
pub struct MediaMovil<const N: usize> {
    datos: [u32; N],
    idx: usize,
    llenos: usize,
    suma: u64,
}

impl<const N: usize> Default for MediaMovil<N> {
    fn default() -> Self {
        Self { datos: [0; N], idx: 0, llenos: 0, suma: 0 }
    }
}

impl<const N: usize> MediaMovil<N> {
    /// Agrega una muestra y devuelve el promedio actualizado.
    pub fn agregar(&mut self, v: u32) -> u32 {
        // Restamos el valor que sale de la ventana y sumamos el nuevo: O(1).
        self.suma -= self.datos[self.idx] as u64;
        self.datos[self.idx] = v;
        self.suma += v as u64;
        self.idx = (self.idx + 1) % N;
        self.llenos = (self.llenos + 1).min(N);
        self.promedio()
    }
    /// Promedio actual (0 si está vacía).
    pub fn promedio(&self) -> u32 {
        if self.llenos == 0 {
            0
        } else {
            (self.suma / self.llenos as u64) as u32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ventana_desliza() {
        let mut m = MediaMovil::<3>::default();
        assert_eq!(m.promedio(), 0);
        assert_eq!(m.agregar(3), 3);
        assert_eq!(m.agregar(6), 4);
        assert_eq!(m.agregar(9), 6);
        assert_eq!(m.agregar(30), 15); // sale el 3
    }
}
