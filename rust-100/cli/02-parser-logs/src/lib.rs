//! # Parser de logs — lógica
//!
//! Formato soportado (muy común en servicios):
//! ```text
//! 2024-05-01T10:15:30 ERROR [db] conexión rechazada
//! 2024-05-01 10:16:02 WARN  reintentando
//! ```
//! Es decir: `FECHA[T| ]HORA NIVEL mensaje`. Las líneas que no encajan se
//! cuentan como "no parseables" (no se descartan en silencio).
//!
//! Componentes:
//! - [`parsear_linea`]: texto → [`Entrada`].
//! - [`Filtro`]: criterios combinables (nivel mínimo, rango de fechas, regex).
//! - [`Estadisticas`]: conteo por nivel y errores por hora.

use regex::Regex;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// Niveles de log ordenados por severidad (el `derive(PartialOrd)` respeta este orden).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Nivel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl FromStr for Nivel {
    type Err = String;
    /// Acepta mayúsculas/minúsculas y alias habituales (WARNING, ERR, FATAL).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "TRACE" => Ok(Nivel::Trace),
            "DEBUG" => Ok(Nivel::Debug),
            "INFO" => Ok(Nivel::Info),
            "WARN" | "WARNING" => Ok(Nivel::Warn),
            "ERROR" | "ERR" | "FATAL" => Ok(Nivel::Error),
            otro => Err(format!("nivel desconocido: {otro}")),
        }
    }
}

impl fmt::Display for Nivel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Nivel::Trace => "TRACE",
            Nivel::Debug => "DEBUG",
            Nivel::Info => "INFO",
            Nivel::Warn => "WARN",
            Nivel::Error => "ERROR",
        };
        f.write_str(s)
    }
}

/// Una línea de log ya parseada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// Fecha `AAAA-MM-DD`.
    pub fecha: String,
    /// Hora `HH:MM:SS`.
    pub hora: String,
    pub nivel: Nivel,
    pub mensaje: String,
}

impl Entrada {
    /// Clave ordenable "AAAA-MM-DD HH:MM:SS" (el formato ISO ordena lexicográficamente).
    pub fn marca(&self) -> String {
        format!("{} {}", self.fecha, self.hora)
    }
    /// Cubeta horaria "AAAA-MM-DD HH:00" para las estadísticas por hora.
    pub fn hora_cubeta(&self) -> String {
        format!("{} {}:00", self.fecha, &self.hora[..2])
    }
}

/// Regex del formato de línea. Se compila una vez por llamada a [`Parser::new`].
const PATRON: &str = r"^(\d{4}-\d{2}-\d{2})[T ](\d{2}:\d{2}:\d{2})(?:\.\d+)?Z?\s+([A-Za-z]+)\s+(.*)$";

/// Parser reutilizable (evita recompilar la regex por línea).
pub struct Parser {
    re: Regex,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    pub fn new() -> Self {
        Parser { re: Regex::new(PATRON).expect("regex fija válida") }
    }

    /// Intenta parsear una línea; `None` si no respeta el formato o el nivel es desconocido.
    pub fn parsear_linea(&self, linea: &str) -> Option<Entrada> {
        let c = self.re.captures(linea.trim_end())?;
        let nivel: Nivel = c[3].parse().ok()?;
        Some(Entrada { fecha: c[1].to_string(), hora: c[2].to_string(), nivel, mensaje: c[4].trim().to_string() })
    }
}

/// Atajo sin estado (compila la regex cada vez: usar [`Parser`] en bucles).
pub fn parsear_linea(linea: &str) -> Option<Entrada> {
    Parser::new().parsear_linea(linea)
}

/// Criterios de filtrado. Todos los campos son opcionales y se combinan con AND.
#[derive(Debug, Default)]
pub struct Filtro {
    /// Nivel mínimo (incluido): `Warn` deja pasar WARN y ERROR.
    pub nivel_min: Option<Nivel>,
    /// Desde (incluido), prefijo comparable: "2024-05-01" o "2024-05-01 10:00:00".
    pub desde: Option<String>,
    /// Hasta (incluido), mismo formato que `desde`.
    pub hasta: Option<String>,
    /// Regex que debe aparecer en el mensaje.
    pub patron: Option<Regex>,
}

impl Filtro {
    /// Evalúa si una entrada cumple TODOS los criterios.
    pub fn acepta(&self, e: &Entrada) -> bool {
        if let Some(n) = self.nivel_min {
            if e.nivel < n {
                return false;
            }
        }
        let marca = e.marca();
        if let Some(d) = &self.desde {
            // Comparación lexicográfica: válida por ser formato ISO-8601.
            if marca.as_str() < d.as_str() {
                return false;
            }
        }
        if let Some(h) = &self.hasta {
            // Para "hasta" con solo fecha comparamos el prefijo, así incluye todo ese día.
            let corte = &marca[..h.len().min(marca.len())];
            if corte > h.as_str() {
                return false;
            }
        }
        if let Some(p) = &self.patron {
            if !p.is_match(&e.mensaje) {
                return false;
            }
        }
        true
    }
}

/// Estadísticas acumuladas tras recorrer el log.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Estadisticas {
    /// Total de líneas leídas.
    pub lineas: usize,
    /// Líneas que no respetan el formato.
    pub no_parseables: usize,
    /// Entradas que pasaron el filtro.
    pub coincidencias: usize,
    /// Conteo por nivel (BTreeMap → orden estable al imprimir).
    pub por_nivel: BTreeMap<Nivel, usize>,
    /// Cantidad de ERROR por cubeta horaria.
    pub errores_por_hora: BTreeMap<String, usize>,
}

impl Estadisticas {
    /// Informe de texto legible.
    pub fn informe(&self) -> String {
        let mut s = format!(
            "Líneas: {} | coincidencias: {} | no parseables: {}\nPor nivel:\n",
            self.lineas, self.coincidencias, self.no_parseables
        );
        for (n, c) in &self.por_nivel {
            s.push_str(&format!("  {n:<5} {c}\n"));
        }
        if !self.errores_por_hora.is_empty() {
            s.push_str("Errores por hora:\n");
            let max = *self.errores_por_hora.values().max().unwrap_or(&1);
            for (h, c) in &self.errores_por_hora {
                // Mini histograma con barras proporcionales (máx. 30 caracteres).
                let barra = "█".repeat((c * 30).div_ceil(max));
                s.push_str(&format!("  {h}  {c:>4} {barra}\n"));
            }
        }
        s
    }
}

/// Procesa un iterador de líneas: filtra y acumula estadísticas.
/// Devuelve las entradas aceptadas (para imprimirlas) y las estadísticas.
pub fn procesar<'a, I: IntoIterator<Item = &'a str>>(lineas: I, filtro: &Filtro) -> (Vec<Entrada>, Estadisticas) {
    let parser = Parser::new();
    let mut st = Estadisticas::default();
    let mut aceptadas = Vec::new();
    for l in lineas {
        st.lineas += 1;
        let Some(e) = parser.parsear_linea(l) else {
            // Líneas vacías no cuentan como "rotas".
            if !l.trim().is_empty() {
                st.no_parseables += 1;
            }
            continue;
        };
        if !filtro.acepta(&e) {
            continue;
        }
        st.coincidencias += 1;
        *st.por_nivel.entry(e.nivel).or_default() += 1;
        if e.nivel == Nivel::Error {
            *st.errores_por_hora.entry(e.hora_cubeta()).or_default() += 1;
        }
        aceptadas.push(e);
    }
    (aceptadas, st)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "2024-05-01T10:15:30 ERROR [db] conexión rechazada
2024-05-01 10:16:02 WARN  reintentando
2024-05-01T10:59:59.123Z info listo
basura sin formato

2024-05-01T11:00:00 ERROR timeout en /api
2024-05-02T09:00:00 DEBUG arranque";

    #[test]
    fn parsea_variantes() {
        let e = parsear_linea("2024-05-01T10:15:30 ERROR [db] x").unwrap();
        assert_eq!(e.nivel, Nivel::Error);
        assert_eq!(e.mensaje, "[db] x");
        assert!(parsear_linea("2024-05-01 10:15:30.5 warning y").is_some());
        assert!(parsear_linea("hola").is_none());
        assert!(parsear_linea("2024-05-01 10:15:30 RARO y").is_none());
    }

    #[test]
    fn estadisticas_sin_filtro() {
        let (v, st) = procesar(LOG.lines(), &Filtro::default());
        assert_eq!(v.len(), 5);
        assert_eq!(st.no_parseables, 1);
        assert_eq!(st.por_nivel[&Nivel::Error], 2);
        assert_eq!(st.errores_por_hora["2024-05-01 10:00"], 1);
        assert_eq!(st.errores_por_hora["2024-05-01 11:00"], 1);
    }

    #[test]
    fn filtra_por_nivel() {
        let f = Filtro { nivel_min: Some(Nivel::Warn), ..Default::default() };
        let (v, _) = procesar(LOG.lines(), &f);
        assert_eq!(v.len(), 3);
    }

    #[test]
    fn filtra_por_fecha() {
        let f = Filtro { desde: Some("2024-05-01 10:16:00".into()), hasta: Some("2024-05-01".into()), ..Default::default() };
        let (v, _) = procesar(LOG.lines(), &f);
        assert_eq!(v.len(), 3); // WARN, info, ERROR 11:00 (excluye el 2024-05-02)
    }

    #[test]
    fn filtra_por_patron() {
        let f = Filtro { patron: Some(Regex::new("(?i)timeout").unwrap()), ..Default::default() };
        let (v, st) = procesar(LOG.lines(), &f);
        assert_eq!(v.len(), 1);
        assert!(st.informe().contains("Errores por hora"));
    }

    #[test]
    fn orden_de_niveles() {
        assert!(Nivel::Error > Nivel::Warn && Nivel::Warn > Nivel::Info);
        assert_eq!("fatal".parse::<Nivel>().unwrap(), Nivel::Error);
    }
}
