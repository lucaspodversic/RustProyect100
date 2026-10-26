//! # To-Do CLI — modelo y persistencia
//!
//! - [`Tarea`] / [`Prioridad`]: el modelo, serializable con serde.
//! - [`Almacen`]: colección en memoria con IDs incrementales y operaciones
//!   (agregar, completar, reabrir, editar, borrar, listar con filtros).
//! - [`Almacen::cargar`] / [`Almacen::guardar`]: persistencia en un archivo JSON
//!   con **escritura atómica** (archivo temporal + `rename`), así un corte
//!   a mitad de escritura no corrompe la base.
//!
//! Elegimos JSON (y no SQLite) porque es legible, versionable con git y
//! suficiente para miles de tareas.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::str::FromStr;

/// Prioridad de una tarea (orden: Alta > Media > Baja).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Prioridad {
    Baja,
    #[default]
    Media,
    Alta,
}

impl FromStr for Prioridad {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "baja" | "b" | "low" => Ok(Prioridad::Baja),
            "media" | "m" | "medium" => Ok(Prioridad::Media),
            "alta" | "a" | "high" => Ok(Prioridad::Alta),
            o => Err(format!("prioridad inválida: {o} (baja|media|alta)")),
        }
    }
}

/// Una tarea.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tarea {
    pub id: u32,
    pub titulo: String,
    pub hecha: bool,
    #[serde(default)]
    pub prioridad: Prioridad,
    /// Fecha límite `AAAA-MM-DD` (validada al cargar desde la CLI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vence: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub etiquetas: Vec<String>,
}

impl fmt::Display for Tarea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let marca = if self.hecha { "[x]" } else { "[ ]" };
        let prio = match self.prioridad {
            Prioridad::Alta => "!!!",
            Prioridad::Media => "!! ",
            Prioridad::Baja => "!  ",
        };
        write!(f, "{:>3} {marca} {prio} {}", self.id, self.titulo)?;
        if let Some(v) = &self.vence {
            write!(f, "  (vence {v})")?;
        }
        if !self.etiquetas.is_empty() {
            write!(f, "  #{}", self.etiquetas.join(" #"))?;
        }
        Ok(())
    }
}

/// Valida una fecha `AAAA-MM-DD` sin dependencias (incluye años bisiestos).
pub fn validar_fecha(s: &str) -> Result<(), String> {
    let p: Vec<&str> = s.split('-').collect();
    let malo = || format!("fecha inválida: {s} (formato AAAA-MM-DD)");
    if p.len() != 3 || p[0].len() != 4 || p[1].len() != 2 || p[2].len() != 2 {
        return Err(malo());
    }
    let (a, m, d): (u32, u32, u32) = (p[0].parse().map_err(|_| malo())?, p[1].parse().map_err(|_| malo())?, p[2].parse().map_err(|_| malo())?);
    let bisiesto = (a % 4 == 0 && a % 100 != 0) || a % 400 == 0;
    let dias = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if bisiesto => 29,
        2 => 28,
        _ => return Err(malo()),
    };
    if d == 0 || d > dias {
        return Err(malo());
    }
    Ok(())
}

/// Filtros para listar.
#[derive(Debug, Default, Clone)]
pub struct Filtro {
    /// `None` = todas, `Some(false)` = pendientes, `Some(true)` = hechas.
    pub hechas: Option<bool>,
    pub etiqueta: Option<String>,
    pub prioridad_min: Option<Prioridad>,
    /// Solo las que vencen hasta esta fecha (incluida).
    pub vence_hasta: Option<String>,
    pub texto: Option<String>,
}

/// Archivo completo: siguiente ID + tareas.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Almacen {
    pub siguiente_id: u32,
    pub tareas: Vec<Tarea>,
}

impl Almacen {
    /// Carga desde disco; si el archivo no existe, devuelve un almacén vacío.
    pub fn cargar(ruta: &Path) -> io::Result<Self> {
        match fs::read_to_string(ruta) {
            Ok(s) => serde_json::from_str(&s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("JSON corrupto: {e}"))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Almacen { siguiente_id: 1, tareas: vec![] }),
            Err(e) => Err(e),
        }
    }

    /// Guarda de forma atómica: escribe `<ruta>.tmp` y lo renombra encima.
    pub fn guardar(&self, ruta: &Path) -> io::Result<()> {
        if let Some(dir) = ruta.parent() {
            if !dir.as_os_str().is_empty() {
                fs::create_dir_all(dir)?;
            }
        }
        let tmp = ruta.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        fs::rename(&tmp, ruta) // rename es atómico dentro del mismo sistema de archivos
    }

    /// Agrega y devuelve el ID asignado.
    pub fn agregar(&mut self, titulo: &str, prioridad: Prioridad, vence: Option<String>, etiquetas: Vec<String>) -> Result<u32, String> {
        let titulo = titulo.trim();
        if titulo.is_empty() {
            return Err("el título no puede estar vacío".into());
        }
        if let Some(v) = &vence {
            validar_fecha(v)?;
        }
        let id = self.siguiente_id.max(1);
        self.siguiente_id = id + 1;
        let etiquetas = etiquetas.into_iter().map(|e| e.trim_start_matches('#').to_lowercase()).collect();
        self.tareas.push(Tarea { id, titulo: titulo.into(), hecha: false, prioridad, vence, etiquetas });
        Ok(id)
    }

    fn buscar(&mut self, id: u32) -> Result<&mut Tarea, String> {
        self.tareas.iter_mut().find(|t| t.id == id).ok_or_else(|| format!("no existe la tarea {id}"))
    }

    /// Marca como hecha (o pendiente con `hecha = false`).
    pub fn marcar(&mut self, id: u32, hecha: bool) -> Result<(), String> {
        self.buscar(id)?.hecha = hecha;
        Ok(())
    }

    /// Edita título y/o prioridad.
    pub fn editar(&mut self, id: u32, titulo: Option<String>, prioridad: Option<Prioridad>) -> Result<(), String> {
        let t = self.buscar(id)?;
        if let Some(x) = titulo {
            t.titulo = x;
        }
        if let Some(p) = prioridad {
            t.prioridad = p;
        }
        Ok(())
    }

    pub fn borrar(&mut self, id: u32) -> Result<Tarea, String> {
        let i = self.tareas.iter().position(|t| t.id == id).ok_or_else(|| format!("no existe la tarea {id}"))?;
        Ok(self.tareas.remove(i))
    }

    /// Borra todas las completadas; devuelve cuántas.
    pub fn limpiar_hechas(&mut self) -> usize {
        let antes = self.tareas.len();
        self.tareas.retain(|t| !t.hecha);
        antes - self.tareas.len()
    }

    /// Lista aplicando filtros; ordena: pendientes primero, prioridad desc, vencimiento asc, id.
    pub fn listar(&self, f: &Filtro) -> Vec<&Tarea> {
        let mut v: Vec<&Tarea> = self
            .tareas
            .iter()
            .filter(|t| f.hechas.is_none_or(|h| t.hecha == h))
            .filter(|t| f.etiqueta.as_ref().is_none_or(|e| t.etiquetas.contains(&e.to_lowercase())))
            .filter(|t| f.prioridad_min.is_none_or(|p| t.prioridad >= p))
            .filter(|t| f.vence_hasta.as_ref().is_none_or(|h| t.vence.as_ref().is_some_and(|v| v <= h)))
            .filter(|t| f.texto.as_ref().is_none_or(|x| t.titulo.to_lowercase().contains(&x.to_lowercase())))
            .collect();
        v.sort_by(|a, b| {
            a.hecha
                .cmp(&b.hecha)
                .then(b.prioridad.cmp(&a.prioridad))
                // `None` al final: comparamos (es_none, fecha).
                .then((a.vence.is_none(), &a.vence).cmp(&(b.vence.is_none(), &b.vence)))
                .then(a.id.cmp(&b.id))
        });
        v
    }

    /// (total, hechas, pendientes)
    pub fn resumen(&self) -> (usize, usize, usize) {
        let h = self.tareas.iter().filter(|t| t.hecha).count();
        (self.tareas.len(), h, self.tareas.len() - h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Almacen {
        let mut a = Almacen::default();
        a.agregar("Comprar pan", Prioridad::Baja, None, vec![]).unwrap();
        a.agregar("Pagar luz", Prioridad::Alta, Some("2024-06-10".into()), vec!["#Casa".into()]).unwrap();
        a.agregar("Estudiar Rust", Prioridad::Media, Some("2024-06-01".into()), vec!["estudio".into()]).unwrap();
        a
    }

    #[test]
    fn agrega_con_ids_incrementales() {
        let a = base();
        assert_eq!(a.tareas.iter().map(|t| t.id).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(a.tareas[1].etiquetas, vec!["casa"]);
    }

    #[test]
    fn valida_entradas() {
        let mut a = Almacen::default();
        assert!(a.agregar("  ", Prioridad::Media, None, vec![]).is_err());
        assert!(a.agregar("x", Prioridad::Media, Some("2024-02-30".into()), vec![]).is_err());
        assert!(validar_fecha("2024-02-29").is_ok());
        assert!(validar_fecha("2023-02-29").is_err());
        assert!(validar_fecha("24-1-1").is_err());
        assert!("ALTA".parse::<Prioridad>().is_ok());
    }

    #[test]
    fn ordena_y_filtra() {
        let mut a = base();
        a.marcar(2, true).unwrap();
        let v = a.listar(&Filtro::default());
        assert_eq!(v.iter().map(|t| t.id).collect::<Vec<_>>(), vec![3, 1, 2]);
        assert_eq!(a.listar(&Filtro { hechas: Some(false), ..Default::default() }).len(), 2);
        assert_eq!(a.listar(&Filtro { etiqueta: Some("CASA".into()), ..Default::default() })[0].id, 2);
        assert_eq!(a.listar(&Filtro { vence_hasta: Some("2024-06-05".into()), ..Default::default() })[0].id, 3);
        assert_eq!(a.listar(&Filtro { prioridad_min: Some(Prioridad::Media), ..Default::default() }).len(), 2);
        assert_eq!(a.listar(&Filtro { texto: Some("rust".into()), ..Default::default() }).len(), 1);
    }

    #[test]
    fn editar_borrar_limpiar() {
        let mut a = base();
        a.editar(1, Some("Pan integral".into()), Some(Prioridad::Alta)).unwrap();
        assert_eq!(a.tareas[0].titulo, "Pan integral");
        assert!(a.borrar(99).is_err());
        a.borrar(1).unwrap();
        a.marcar(3, true).unwrap();
        assert_eq!(a.limpiar_hechas(), 1);
        assert_eq!(a.resumen(), (1, 0, 1));
        // Los IDs no se reutilizan tras borrar.
        assert_eq!(a.agregar("nueva", Prioridad::Media, None, vec![]).unwrap(), 4);
    }

    #[test]
    fn persistencia_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("sub/todo.json");
        assert_eq!(Almacen::cargar(&p).unwrap().tareas.len(), 0);
        let a = base();
        a.guardar(&p).unwrap();
        assert_eq!(Almacen::cargar(&p).unwrap(), a);
        assert!(!p.with_extension("json.tmp").exists());
        fs::write(&p, "{roto").unwrap();
        assert!(Almacen::cargar(&p).is_err());
    }

    #[test]
    fn display() {
        let a = base();
        assert_eq!(a.tareas[1].to_string(), "  2 [ ] !!! Pagar luz  (vence 2024-06-10)  #casa");
    }
}
