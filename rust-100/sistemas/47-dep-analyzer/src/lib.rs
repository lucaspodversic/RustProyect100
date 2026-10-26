//! # Analizador de dependencias
//!
//! Grafo dirigido crate/módulo → dependencias. Detecta **ciclos** con
//! DFS (colores) y produce un orden topológico si el grafo es un DAG.

use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Default, Clone)]
pub struct Graph {
    /// nodo → conjunto de dependencias (aristas salientes).
    pub edges: BTreeMap<String, BTreeSet<String>>,
}

impl Graph {
    pub fn new() -> Self { Self::default() }

    pub fn add_node(&mut self, n: &str) {
        self.edges.entry(n.into()).or_default();
    }

    pub fn add_edge(&mut self, from: &str, to: &str) {
        self.add_node(from);
        self.add_node(to);
        self.edges.get_mut(from).unwrap().insert(to.into());
    }

    /// Parsea un formato simple: cada línea `a -> b, c, d`.
    pub fn parse(text: &str) -> Self {
        let mut g = Self::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((from, rest)) = line.split_once("->") {
                let from = from.trim();
                g.add_node(from);
                for to in rest.split(',') {
                    let to = to.trim();
                    if !to.is_empty() {
                        g.add_edge(from, to);
                    }
                }
            } else {
                g.add_node(line);
            }
        }
        g
    }

    /// Ciclos encontrados (cada ciclo es una lista de nodos que vuelve al inicio).
    pub fn find_cycles(&self) -> Vec<Vec<String>> {
        let mut color: HashMap<&str, u8> = HashMap::new(); // 0=blanco 1=gris 2=negro
        let mut stack = Vec::new();
        let mut cycles = Vec::new();
        for n in self.edges.keys() {
            if color.get(n.as_str()).copied().unwrap_or(0) == 0 {
                self.dfs(n, &mut color, &mut stack, &mut cycles);
            }
        }
        cycles
    }

    fn dfs<'a>(
        &'a self,
        n: &'a str,
        color: &mut HashMap<&'a str, u8>,
        stack: &mut Vec<&'a str>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        color.insert(n, 1);
        stack.push(n);
        if let Some(deps) = self.edges.get(n) {
            for d in deps {
                match color.get(d.as_str()).copied().unwrap_or(0) {
                    0 => self.dfs(d, color, stack, cycles),
                    1 => {
                        // Ciclo: desde la aparición previa de d hasta el tope.
                        let start = stack.iter().rposition(|x| *x == d.as_str()).unwrap();
                        let mut c: Vec<String> = stack[start..].iter().map(|s| (*s).to_string()).collect();
                        c.push(d.clone());
                        cycles.push(c);
                    }
                    _ => {}
                }
            }
        }
        stack.pop();
        color.insert(n, 2);
    }

    /// Orden topológico (Kahn). `None` si hay ciclo.
    /// Orden de *build*: las dependencias aparecen antes que quien las usa.
    /// Arista `A -> B` significa "A depende de B" ⇒ B antes que A.
    pub fn topo_sort(&self) -> Option<Vec<String>> {
        // Invertimos: construimos lista de "quién depende de mí".
        let mut dependents: HashMap<&str, Vec<&str>> = self.edges.keys().map(|k| (k.as_str(), Vec::new())).collect();
        let mut indeg: HashMap<&str, usize> = self.edges.keys().map(|k| (k.as_str(), 0)).collect();
        for (from, deps) in &self.edges {
            for d in deps {
                dependents.entry(d.as_str()).or_default().push(from.as_str());
                *indeg.get_mut(from.as_str()).unwrap() += 1;
            }
        }
        let mut q: Vec<&str> = indeg.iter().filter(|(_, &d)| d == 0).map(|(&k, _)| k).collect();
        q.sort();
        let mut out = Vec::new();
        while let Some(n) = {
            // sacamos el primero (orden lexicográfico estable)
            if q.is_empty() { None } else { Some(q.remove(0)) }
        } {
            out.push(n.to_string());
            for &dep in dependents.get(n).into_iter().flatten() {
                let e = indeg.get_mut(dep).unwrap();
                *e -= 1;
                if *e == 0 {
                    q.push(dep);
                    q.sort();
                }
            }
        }
        if out.len() == self.edges.len() { Some(out) } else { None }
    }

    /// Representación DOT para Graphviz.
    pub fn to_dot(&self) -> String {
        let mut s = String::from("digraph deps {\n");
        for (from, tos) in &self.edges {
            if tos.is_empty() {
                s += &format!("  \"{from}\";\n");
            }
            for to in tos {
                s += &format!("  \"{from}\" -> \"{to}\";\n");
            }
        }
        s + "}\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detecta_ciclo() {
        let g = Graph::parse("a -> b\nb -> c\nc -> a\nd -> a");
        let cycles = g.find_cycles();
        assert!(!cycles.is_empty());
        assert!(g.topo_sort().is_none());
    }
    #[test]
    fn topo_ok() {
        let g = Graph::parse("app -> lib, util\nlib -> util\nutil");
        let t = g.topo_sort().unwrap();
        // util antes que lib antes que app
        let pos: HashMap<_, _> = t.iter().enumerate().map(|(i, n)| (n.clone(), i)).collect();
        assert!(pos["util"] < pos["lib"]);
        assert!(pos["lib"] < pos["app"]);
        assert!(g.to_dot().contains("app"));
    }
}
