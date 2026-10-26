//! # Blockchain mínima (proyecto 66)
//!
//! Implementa los tres pilares de cualquier cadena de bloques:
//!
//! 1. **Bloques encadenados**: cada bloque guarda el hash del anterior, de modo que
//!    alterar un bloque viejo rompe todos los enlaces posteriores.
//! 2. **Prueba de Trabajo (PoW)**: para "sellar" un bloque hay que encontrar un
//!    `nonce` tal que el hash SHA-256 del encabezado empiece con `dificultad`
//!    ceros en hexadecimal. Encontrarlo es caro; verificarlo es barato.
//! 3. **Validación de la cadena**: recorremos todos los bloques comprobando
//!    índices, enlaces, hashes recalculados, PoW y marcas de tiempo.
//!
//! Todo es determinista y sin dependencias de red para que los tests sean
//! reproducibles.

use sha2::{Digest, Sha256};
use std::fmt;

/// Un bloque de la cadena.
///
/// Los datos se guardan como `String` para mantener el ejemplo simple; en una
/// cadena real serían una lista de transacciones resumida con un árbol de Merkle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bloque {
    /// Posición del bloque en la cadena (el génesis es 0).
    pub indice: u64,
    /// Marca de tiempo en segundos (la pasamos explícita para que los tests sean deterministas).
    pub timestamp: u64,
    /// Carga útil del bloque.
    pub datos: String,
    /// Hash (hex) del bloque anterior. El génesis usa 64 ceros.
    pub hash_previo: String,
    /// Número que el minero va probando hasta cumplir la dificultad.
    pub nonce: u64,
    /// Cantidad de ceros hexadecimales iniciales exigidos al hash.
    pub dificultad: u32,
    /// Hash (hex) del encabezado, calculado tras minar.
    pub hash: String,
}

impl Bloque {
    /// Serializa los campos del encabezado de forma canónica y calcula su SHA-256.
    ///
    /// Importante: el propio campo `hash` NO entra en el cálculo (sería circular).
    pub fn calcular_hash(&self) -> String {
        let mut h = Sha256::new();
        // Usamos bytes little-endian de longitud fija para los enteros y un
        // prefijo de longitud para los datos: así dos encabezados distintos
        // nunca producen la misma serialización (evita ambigüedades).
        h.update(self.indice.to_le_bytes());
        h.update(self.timestamp.to_le_bytes());
        h.update((self.datos.len() as u64).to_le_bytes());
        h.update(self.datos.as_bytes());
        h.update(self.hash_previo.as_bytes());
        h.update(self.nonce.to_le_bytes());
        h.update(self.dificultad.to_le_bytes());
        hex::encode(h.finalize())
    }

    /// Devuelve `true` si `hash` empieza con `dificultad` ceros hexadecimales.
    pub fn cumple_dificultad(hash: &str, dificultad: u32) -> bool {
        hash.len() >= dificultad as usize && hash.bytes().take(dificultad as usize).all(|b| b == b'0')
    }

    /// Mina el bloque: incrementa `nonce` desde 0 hasta encontrar un hash válido.
    ///
    /// Devuelve la cantidad de intentos realizados (útil para estadísticas).
    pub fn minar(&mut self) -> u64 {
        self.nonce = 0;
        loop {
            let candidato = self.calcular_hash();
            if Self::cumple_dificultad(&candidato, self.dificultad) {
                self.hash = candidato;
                return self.nonce + 1;
            }
            // `wrapping_add` evita pánicos teóricos por desbordamiento.
            self.nonce = self.nonce.wrapping_add(1);
        }
    }
}

impl fmt::Display for Bloque {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{} ts={} nonce={} dif={} hash={}… prev={}… datos={:?}",
            self.indice,
            self.timestamp,
            self.nonce,
            self.dificultad,
            &self.hash[..12.min(self.hash.len())],
            &self.hash_previo[..12.min(self.hash_previo.len())],
            self.datos
        )
    }
}

/// Errores posibles al validar una cadena. Cada variante indica el índice culpable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorValidacion {
    /// La cadena está vacía (ni siquiera tiene génesis).
    CadenaVacia,
    /// El génesis no tiene el formato esperado.
    GenesisInvalido,
    /// El índice no es consecutivo.
    IndiceIncorrecto(u64),
    /// `hash_previo` no coincide con el hash del bloque anterior.
    EnlaceRoto(u64),
    /// El hash guardado no coincide con el recalculado (datos manipulados).
    HashManipulado(u64),
    /// El hash no cumple la dificultad declarada.
    PowInsuficiente(u64),
    /// La marca de tiempo retrocede respecto del bloque anterior.
    TiempoInvalido(u64),
}

impl fmt::Display for ErrorValidacion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CadenaVacia => write!(f, "la cadena está vacía"),
            Self::GenesisInvalido => write!(f, "bloque génesis inválido"),
            Self::IndiceIncorrecto(i) => write!(f, "índice incorrecto en bloque {i}"),
            Self::EnlaceRoto(i) => write!(f, "enlace roto en bloque {i}"),
            Self::HashManipulado(i) => write!(f, "hash manipulado en bloque {i}"),
            Self::PowInsuficiente(i) => write!(f, "PoW insuficiente en bloque {i}"),
            Self::TiempoInvalido(i) => write!(f, "timestamp retrocede en bloque {i}"),
        }
    }
}

impl std::error::Error for ErrorValidacion {}

/// La cadena de bloques en sí: un vector ordenado más la dificultad vigente.
#[derive(Debug, Clone)]
pub struct Cadena {
    pub bloques: Vec<Bloque>,
    /// Dificultad que se aplicará al próximo bloque minado.
    pub dificultad: u32,
    /// Tiempo objetivo entre bloques (segundos) para el ajuste de dificultad.
    pub tiempo_objetivo: u64,
    /// Cada cuántos bloques se reajusta la dificultad.
    pub ventana_ajuste: u64,
}

/// Hash previo del génesis: 64 ceros (no hay bloque anterior).
pub const HASH_CERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

impl Cadena {
    /// Crea una cadena nueva y mina su bloque génesis.
    pub fn nueva(dificultad: u32, timestamp_genesis: u64) -> Self {
        let mut genesis = Bloque {
            indice: 0,
            timestamp: timestamp_genesis,
            datos: "génesis".to_string(),
            hash_previo: HASH_CERO.to_string(),
            nonce: 0,
            dificultad,
            hash: String::new(),
        };
        genesis.minar();
        Self { bloques: vec![genesis], dificultad, tiempo_objetivo: 10, ventana_ajuste: 5 }
    }

    /// Último bloque (siempre existe porque la cadena nace con génesis).
    pub fn ultimo(&self) -> &Bloque {
        self.bloques.last().expect("la cadena siempre tiene génesis")
    }

    /// Mina y agrega un bloque nuevo con los datos y timestamp indicados.
    pub fn agregar(&mut self, datos: impl Into<String>, timestamp: u64) -> &Bloque {
        let previo = self.ultimo();
        let mut b = Bloque {
            indice: previo.indice + 1,
            // Nunca dejamos que el tiempo retroceda: tomamos el máximo.
            timestamp: timestamp.max(previo.timestamp),
            datos: datos.into(),
            hash_previo: previo.hash.clone(),
            nonce: 0,
            dificultad: self.dificultad,
            hash: String::new(),
        };
        b.minar();
        self.bloques.push(b);
        self.ajustar_dificultad();
        self.ultimo()
    }

    /// Ajuste de dificultad estilo Bitcoin simplificado.
    ///
    /// Cada `ventana_ajuste` bloques comparamos el tiempo real transcurrido con
    /// el esperado: si fue la mitad o menos, subimos la dificultad en 1; si fue
    /// el doble o más, la bajamos (mínimo 1).
    pub fn ajustar_dificultad(&mut self) {
        let n = self.bloques.len() as u64;
        if n <= self.ventana_ajuste || (n - 1) % self.ventana_ajuste != 0 {
            return;
        }
        let fin = self.ultimo().timestamp;
        let ini = self.bloques[(n - 1 - self.ventana_ajuste) as usize].timestamp;
        let real = fin.saturating_sub(ini);
        let esperado = self.tiempo_objetivo * self.ventana_ajuste;
        if real * 2 <= esperado {
            self.dificultad += 1;
        } else if real >= esperado * 2 && self.dificultad > 1 {
            self.dificultad -= 1;
        }
    }

    /// Valida la cadena completa. Devuelve el primer error encontrado.
    pub fn validar(&self) -> Result<(), ErrorValidacion> {
        validar_bloques(&self.bloques)
    }

    /// "Trabajo acumulado" aproximado: suma de 16^dificultad de cada bloque.
    /// Es el criterio para elegir entre dos cadenas válidas (la más pesada gana).
    pub fn trabajo_total(&self) -> u128 {
        self.bloques.iter().map(|b| 16u128.saturating_pow(b.dificultad)).sum()
    }

    /// Regla de consenso: reemplaza la cadena local si la candidata es válida,
    /// comparte el mismo génesis y tiene más trabajo acumulado.
    pub fn reemplazar_si_mejor(&mut self, candidata: &Cadena) -> bool {
        if candidata.validar().is_err() {
            return false;
        }
        if candidata.bloques[0] != self.bloques[0] {
            return false;
        }
        if candidata.trabajo_total() > self.trabajo_total() {
            self.bloques = candidata.bloques.clone();
            self.dificultad = candidata.dificultad;
            return true;
        }
        false
    }
}

/// Valida una secuencia arbitraria de bloques (reutilizable por otros módulos).
pub fn validar_bloques(bloques: &[Bloque]) -> Result<(), ErrorValidacion> {
    let genesis = bloques.first().ok_or(ErrorValidacion::CadenaVacia)?;
    if genesis.indice != 0 || genesis.hash_previo != HASH_CERO {
        return Err(ErrorValidacion::GenesisInvalido);
    }
    if genesis.calcular_hash() != genesis.hash {
        return Err(ErrorValidacion::HashManipulado(0));
    }
    if !Bloque::cumple_dificultad(&genesis.hash, genesis.dificultad) {
        return Err(ErrorValidacion::PowInsuficiente(0));
    }
    for par in bloques.windows(2) {
        let (prev, act) = (&par[0], &par[1]);
        if act.indice != prev.indice + 1 {
            return Err(ErrorValidacion::IndiceIncorrecto(act.indice));
        }
        if act.hash_previo != prev.hash {
            return Err(ErrorValidacion::EnlaceRoto(act.indice));
        }
        if act.calcular_hash() != act.hash {
            return Err(ErrorValidacion::HashManipulado(act.indice));
        }
        if !Bloque::cumple_dificultad(&act.hash, act.dificultad) {
            return Err(ErrorValidacion::PowInsuficiente(act.indice));
        }
        if act.timestamp < prev.timestamp {
            return Err(ErrorValidacion::TiempoInvalido(act.indice));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: cadena de dificultad 2 con tres bloques.
    fn cadena_base() -> Cadena {
        let mut c = Cadena::nueva(2, 1_000);
        c.agregar("Ana paga 5 a Beto", 1_010);
        c.agregar("Beto paga 2 a Caro", 1_020);
        c
    }

    #[test]
    fn genesis_es_valido_y_cumple_pow() {
        let c = Cadena::nueva(2, 0);
        assert_eq!(c.bloques.len(), 1);
        assert!(c.ultimo().hash.starts_with("00"));
        assert!(c.validar().is_ok());
    }

    #[test]
    fn agregar_enlaza_con_el_anterior() {
        let c = cadena_base();
        assert_eq!(c.bloques[1].hash_previo, c.bloques[0].hash);
        assert_eq!(c.bloques[2].hash_previo, c.bloques[1].hash);
        assert!(c.validar().is_ok());
    }

    #[test]
    fn hash_es_determinista() {
        let c = cadena_base();
        assert_eq!(c.bloques[1].calcular_hash(), c.bloques[1].hash);
    }

    #[test]
    fn manipular_datos_se_detecta() {
        let mut c = cadena_base();
        c.bloques[1].datos = "Ana paga 500 a Beto".into();
        assert_eq!(c.validar(), Err(ErrorValidacion::HashManipulado(1)));
    }

    #[test]
    fn recalcular_hash_sin_reminar_rompe_enlace_o_pow() {
        let mut c = cadena_base();
        c.bloques[1].datos = "trampa".into();
        c.bloques[1].hash = c.bloques[1].calcular_hash();
        let err = c.validar().unwrap_err();
        assert!(matches!(err, ErrorValidacion::PowInsuficiente(1) | ErrorValidacion::EnlaceRoto(2)));
    }

    #[test]
    fn reminar_un_bloque_rompe_el_siguiente_enlace() {
        let mut c = cadena_base();
        c.bloques[1].datos = "trampa".into();
        c.bloques[1].minar();
        assert_eq!(c.validar(), Err(ErrorValidacion::EnlaceRoto(2)));
    }

    #[test]
    fn indice_incorrecto_detectado() {
        let mut c = cadena_base();
        c.bloques[2].indice = 7;
        c.bloques[2].minar();
        assert_eq!(c.validar(), Err(ErrorValidacion::IndiceIncorrecto(7)));
    }

    #[test]
    fn timestamp_no_retrocede_al_agregar() {
        let mut c = Cadena::nueva(1, 100);
        c.agregar("x", 50);
        assert_eq!(c.ultimo().timestamp, 100);
    }

    #[test]
    fn cadena_vacia_es_error() {
        assert_eq!(validar_bloques(&[]), Err(ErrorValidacion::CadenaVacia));
    }

    #[test]
    fn dificultad_sube_si_los_bloques_son_rapidos() {
        let mut c = Cadena::nueva(1, 0);
        for i in 1..=5 {
            c.agregar(format!("b{i}"), i); // 1 segundo entre bloques (objetivo 10)
        }
        assert_eq!(c.dificultad, 2);
        assert!(c.validar().is_ok());
    }

    #[test]
    fn dificultad_baja_si_los_bloques_son_lentos() {
        let mut c = Cadena::nueva(2, 0);
        for i in 1..=5 {
            c.agregar(format!("b{i}"), i * 100);
        }
        assert_eq!(c.dificultad, 1);
    }

    #[test]
    fn consenso_elige_cadena_mas_pesada() {
        let mut local = Cadena::nueva(1, 0);
        let mut remota = local.clone();
        local.agregar("a", 10);
        remota.agregar("a", 10);
        remota.agregar("b", 20);
        assert!(local.reemplazar_si_mejor(&remota));
        assert_eq!(local.bloques.len(), 3);
        // Una cadena con génesis distinto se rechaza.
        let ajena = Cadena::nueva(1, 999);
        assert!(!local.reemplazar_si_mejor(&ajena));
    }

    #[test]
    fn consenso_rechaza_cadena_invalida() {
        let mut local = Cadena::nueva(1, 0);
        let mut remota = local.clone();
        remota.agregar("a", 1);
        remota.agregar("b", 2);
        remota.bloques[1].datos = "x".into();
        assert!(!local.reemplazar_si_mejor(&remota));
    }

    #[test]
    fn cumple_dificultad_borde() {
        assert!(Bloque::cumple_dificultad("00ab", 2));
        assert!(!Bloque::cumple_dificultad("0ab", 2));
        assert!(Bloque::cumple_dificultad("abc", 0));
        assert!(!Bloque::cumple_dificultad("0", 3));
    }
}
