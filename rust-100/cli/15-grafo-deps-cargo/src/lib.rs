//! # Grafo de dependencias de Cargo — lógica
//!
//! Lee un `Cargo.lock` (formato v3/v4, TOML) y construye un grafo dirigido
//! paquete → dependencia. Sobre el grafo ofrece:
//! - [`Grafo::arbol`]: árbol ASCII estilo `cargo tree`, marcando con `(*)` lo ya mostrado.
//! - [`Grafo::dot`]: salida Graphviz DOT (`dot -Tsvg`).
//! - [`Grafo::por_que`]: camino más corto (BFS) desde las raíces hasta un crate:
//!   responde "¿por qué tengo esta dependencia?".
//! - [`Grafo::duplicados`]: crates presentes en más de una versión.
//! - [`Grafo::estadisticas`]: totales y profundidad máxima.
//!
//! Se usa `Cargo.lock` y no `cargo metadata` para funcionar **sin red ni cargo**
//! y para que los tests usen texto fijo. Desventaja: el lock no distingue
//! dependencias normales de dev/build ni features (ver README).

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fmt::Write;

/// Estructura del Cargo.lock tal como viene en el TOML.
#[derive(Deserialize)]
struct Lock {
    #[serde(default)]
    package: Vec<PaqueteLock>,
}
#[derive(Deserialize)]
struct PaqueteLock {
    name: String,
    version: String,
    source: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
}

/// Nodo del grafo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paquete {
    pub nombre: String,
    pub version: String,
    /// `None` = paquete local del workspace (raíz).
    pub fuente: Option<String>,
}
impl Paquete {
    pub fn etiqueta(&self) -> String {
        format!("{} v{}", self.nombre, self.version)
    }
}

/// Grafo con índices numéricos; `deps[i]` son los hijos del nodo `i`.
#[derive(Debug)]
pub struct Grafo {
    pub nodos: Vec<Paquete>,
    pub deps: Vec<Vec<usize>>,
}

/// Errores de carga.
#[derive(Debug)]
pub enum ErrorGrafo {
    Toml(toml::de::Error),
    Referencia(String),
}
impl std::fmt::Display for ErrorGrafo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorGrafo::Toml(e) => write!(f, "Cargo.lock inválido: {e}"),
            ErrorGrafo::Referencia(s) => write!(f, "dependencia no encontrada en el lock: {s}"),
        }
    }
}
impl std::error::Error for ErrorGrafo {}

/// Estadísticas generales.
#[derive(Debug, PartialEq, Eq)]
pub struct Estadisticas {
    pub paquetes: usize,
    pub raices: usize,
    pub aristas: usize,
    pub profundidad_max: usize,
}

impl Grafo {
    /// Parsea el contenido de un Cargo.lock.
    pub fn desde_lock(texto: &str) -> Result<Self, ErrorGrafo> {
        let lock: Lock = toml::from_str(texto).map_err(ErrorGrafo::Toml)?;
        let nodos: Vec<Paquete> = lock
            .package
            .iter()
            .map(|p| Paquete { nombre: p.name.clone(), version: p.version.clone(), fuente: p.source.clone() })
            .collect();
        // Índice nombre → posiciones (puede haber varias versiones del mismo nombre).
        let mut por_nombre: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, n) in nodos.iter().enumerate() {
            por_nombre.entry(&n.nombre).or_default().push(i);
        }
        let mut deps = vec![Vec::new(); nodos.len()];
        for (i, p) in lock.package.iter().enumerate() {
            for d in &p.dependencies {
                // Formatos posibles: "nombre" | "nombre version" | "nombre version (fuente)".
                // El lock solo agrega la versión cuando el nombre es ambiguo.
                let mut partes = d.split_whitespace();
                let nombre = partes.next().unwrap_or_default();
                let version = partes.next();
                let cand = por_nombre.get(nombre).ok_or_else(|| ErrorGrafo::Referencia(d.clone()))?;
                let j = match version {
                    Some(v) => *cand.iter().find(|&&j| nodos[j].version == v).ok_or_else(|| ErrorGrafo::Referencia(d.clone()))?,
                    None => cand[0],
                };
                deps[i].push(j);
            }
            deps[i].sort_by(|&a, &b| nodos[a].nombre.cmp(&nodos[b].nombre));
        }
        Ok(Grafo { nodos, deps })
    }

    /// Paquetes locales (sin `source`): los crates del workspace.
    pub fn raices(&self) -> Vec<usize> {
        (0..self.nodos.len()).filter(|&i| self.nodos[i].fuente.is_none()).collect()
    }

    /// Busca un nodo por nombre (primera versión).
    pub fn buscar(&self, nombre: &str) -> Option<usize> {
        self.nodos.iter().position(|n| n.nombre == nombre)
    }

    /// Árbol ASCII desde `raiz`, con profundidad máxima opcional.
    pub fn arbol(&self, raiz: usize, max: Option<usize>) -> String {
        let mut s = String::new();
        let mut vistos = BTreeSet::new();
        writeln!(s, "{}", self.nodos[raiz].etiqueta()).ok();
        vistos.insert(raiz);
        self.arbol_rec(raiz, "", 1, max, &mut vistos, &mut s);
        s
    }

    fn arbol_rec(&self, n: usize, prefijo: &str, nivel: usize, max: Option<usize>, vistos: &mut BTreeSet<usize>, s: &mut String) {
        if max.is_some_and(|m| nivel > m) {
            return;
        }
        let hijos = &self.deps[n];
        for (k, &h) in hijos.iter().enumerate() {
            let ultimo = k + 1 == hijos.len();
            let repetido = !vistos.insert(h);
            writeln!(s, "{prefijo}{}{}{}", if ultimo { "└── " } else { "├── " }, self.nodos[h].etiqueta(), if repetido && !self.deps[h].is_empty() { " (*)" } else { "" }).ok();
            if !repetido {
                let p = format!("{prefijo}{}", if ultimo { "    " } else { "│   " });
                self.arbol_rec(h, &p, nivel + 1, max, vistos, s);
            }
        }
    }

    /// Graphviz DOT. Raíces en verde, duplicados en naranja.
    pub fn dot(&self) -> String {
        let dup: BTreeSet<&str> = self.duplicados().into_keys().collect();
        let mut s = String::from("digraph deps {\n  rankdir=LR;\n  node [shape=box, fontname=\"monospace\"];\n");
        for (i, n) in self.nodos.iter().enumerate() {
            let color = if n.fuente.is_none() {
                ", style=filled, fillcolor=palegreen"
            } else if dup.contains(n.nombre.as_str()) {
                ", style=filled, fillcolor=orange"
            } else {
                ""
            };
            writeln!(s, "  n{i} [label=\"{}\\n{}\"{color}];", n.nombre, n.version).ok();
        }
        for (i, hs) in self.deps.iter().enumerate() {
            for h in hs {
                writeln!(s, "  n{i} -> n{h};").ok();
            }
        }
        s.push_str("}\n");
        s
    }

    /// Camino más corto raíz → destino (BFS multi-origen).
    pub fn por_que(&self, destino: usize) -> Option<Vec<usize>> {
        let mut previo: Vec<Option<usize>> = vec![None; self.nodos.len()];
        let mut visitado = vec![false; self.nodos.len()];
        let mut cola = VecDeque::new();
        for r in self.raices() {
            visitado[r] = true;
            cola.push_back(r);
        }
        while let Some(n) = cola.pop_front() {
            if n == destino {
                let mut camino = vec![n];
                let mut cur = n;
                while let Some(p) = previo[cur] {
                    camino.push(p);
                    cur = p;
                }
                camino.reverse();
                return Some(camino);
            }
            for &h in &self.deps[n] {
                if !visitado[h] {
                    visitado[h] = true;
                    previo[h] = Some(n);
                    cola.push_back(h);
                }
            }
        }
        None
    }

    /// Nombre → versiones, solo los que tienen 2+ versiones.
    pub fn duplicados(&self) -> BTreeMap<&str, Vec<&str>> {
        let mut m: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for n in &self.nodos {
            m.entry(&n.nombre).or_default().push(&n.version);
        }
        m.retain(|_, v| v.len() > 1);
        m
    }

    /// Profundidad = distancia más larga desde una raíz (DFS con memo; el lock es un DAG).
    pub fn estadisticas(&self) -> Estadisticas {
        fn prof(g: &Grafo, n: usize, memo: &mut Vec<Option<usize>>, en_pila: &mut Vec<bool>) -> usize {
            if let Some(p) = memo[n] {
                return p;
            }
            if en_pila[n] {
                return 0; // ciclo (no debería ocurrir): cortamos
            }
            en_pila[n] = true;
            let p = g.deps[n].iter().map(|&h| 1 + prof(g, h, memo, en_pila)).max().unwrap_or(0);
            en_pila[n] = false;
            memo[n] = Some(p);
            p
        }
        let mut memo = vec![None; self.nodos.len()];
        let mut pila = vec![false; self.nodos.len()];
        let raices = self.raices();
        Estadisticas {
            paquetes: self.nodos.len(),
            raices: raices.len(),
            aristas: self.deps.iter().map(Vec::len).sum(),
            profundidad_max: raices.iter().map(|&r| prof(self, r, &mut memo, &mut pila)).max().unwrap_or(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"
version = 4

[[package]]
name = "mi-app"
version = "0.1.0"
dependencies = ["clap", "serde", "rand 0.8.5"]

[[package]]
name = "clap"
version = "4.5.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = ["clap_builder"]

[[package]]
name = "clap_builder"
version = "4.5.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = ["rand 0.7.3"]

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "rand"
version = "0.7.3"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "rand"
version = "0.8.5"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = ["serde"]
"#;

    #[test]
    fn parsea_y_resuelve_versiones() {
        let g = Grafo::desde_lock(LOCK).unwrap();
        assert_eq!(g.raices(), vec![0]);
        let app = &g.deps[0];
        let vs: Vec<String> = app.iter().map(|&i| g.nodos[i].etiqueta()).collect();
        assert_eq!(vs, vec!["clap v4.5.0", "rand v0.8.5", "serde v1.0.0"]);
    }

    #[test]
    fn arbol_marca_repetidos() {
        let g = Grafo::desde_lock(LOCK).unwrap();
        let a = g.arbol(0, None);
        assert!(a.starts_with("mi-app v0.1.0\n├── clap v4.5.0\n│   └── clap_builder v4.5.0\n│       └── rand v0.7.3\n"));
        // serde aparece dos veces (vía rand 0.8.5 y directa); la segunda no se expande.
        assert_eq!(a.matches("serde v1.0.0").count(), 2);
        let corto = g.arbol(0, Some(1));
        assert!(!corto.contains("clap_builder"));
    }

    #[test]
    fn por_que_encuentra_camino_corto() {
        let g = Grafo::desde_lock(LOCK).unwrap();
        let destino = g.nodos.iter().position(|n| n.etiqueta() == "rand v0.7.3").unwrap();
        let c: Vec<String> = g.por_que(destino).unwrap().iter().map(|&i| g.nodos[i].nombre.clone()).collect();
        assert_eq!(c, vec!["mi-app", "clap", "clap_builder", "rand"]);
    }

    #[test]
    fn duplicados_y_estadisticas() {
        let g = Grafo::desde_lock(LOCK).unwrap();
        assert_eq!(g.duplicados().get("rand").unwrap(), &vec!["0.7.3", "0.8.5"]);
        assert_eq!(g.estadisticas(), Estadisticas { paquetes: 6, raices: 1, aristas: 6, profundidad_max: 3 });
    }

    #[test]
    fn dot_valido() {
        let g = Grafo::desde_lock(LOCK).unwrap();
        let d = g.dot();
        assert!(d.starts_with("digraph deps {"));
        assert_eq!(d.matches("->").count(), 6);
        assert!(d.contains("fillcolor=orange")); // rand duplicado
    }

    #[test]
    fn errores() {
        assert!(matches!(Grafo::desde_lock("no es toml ["), Err(ErrorGrafo::Toml(_))));
        let roto = "[[package]]\nname='a'\nversion='1'\ndependencies=['fantasma']\n";
        assert!(matches!(Grafo::desde_lock(roto), Err(ErrorGrafo::Referencia(_))));
    }

    #[test]
    fn lock_real_de_este_proyecto_si_existe() {
        // Si se compiló dentro de un workspace con Cargo.lock, lo parseamos de verdad.
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock");
        if let Ok(t) = std::fs::read_to_string(p) {
            let g = Grafo::desde_lock(&t).unwrap();
            assert!(g.buscar("grafo-deps-cargo").is_some());
            assert!(g.buscar("toml").is_some());
        }
    }
}
