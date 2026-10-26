//! Generadores de datos de prueba (deterministas, con xorshift64).

/// Distribución de la entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distribucion {
    /// Valores aleatorios uniformes.
    Aleatoria,
    /// Ya ordenada (peor caso del quicksort ingenuo con pivote al extremo).
    Ordenada,
    /// Ordenada al revés.
    Inversa,
    /// Muchos duplicados (solo 16 valores distintos): pone a prueba la
    /// partición en tres vías.
    PocasClaves,
}

/// Genera `n` enteros.
pub fn generar(n: usize, d: Distribucion, semilla: u64) -> Vec<u64> {
    let mut x = semilla | 1;
    let mut rnd = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    match d {
        Distribucion::Aleatoria => (0..n).map(|_| rnd()).collect(),
        Distribucion::Ordenada => (0..n as u64).collect(),
        Distribucion::Inversa => (0..n as u64).rev().collect(),
        Distribucion::PocasClaves => (0..n).map(|_| rnd() % 16).collect(),
    }
}
