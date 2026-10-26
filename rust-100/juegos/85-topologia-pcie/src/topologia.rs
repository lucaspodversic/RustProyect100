//! Árbol PCIe y relaciones entre GPUs (al estilo de una matriz "topo").

use crate::modelo::{Bdf, Dispositivo, Tipo};
use std::collections::HashMap;

/// Relación entre dos dispositivos, de más cercana a más lejana:
/// - `Mismo`: el propio dispositivo.
/// - `Pix`: cuelgan del mismo puente (un solo salto de switch).
/// - `Pxb`: atraviesan varios puentes pero sin pasar por el root complex.
/// - `Phb`: se encuentran recién en el host bridge (mismo root complex).
/// - `Node`: distintos root complex del mismo nodo NUMA.
/// - `Sys`: distinto nodo NUMA (cruzan la interconexión entre CPUs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Relacion {
    Mismo,
    Pix,
    Pxb,
    Phb,
    Node,
    Sys,
}

impl Relacion {
    pub fn sigla(self) -> &'static str {
        ["X", "PIX", "PXB", "PHB", "NODE", "SYS"][self as usize]
    }
}

pub struct Topologia {
    pub dispositivos: Vec<Dispositivo>,
    indice: HashMap<Bdf, usize>,
}

impl Topologia {
    pub fn new(dispositivos: Vec<Dispositivo>) -> Self {
        let indice = dispositivos.iter().enumerate().map(|(i, d)| (d.bdf.clone(), i)).collect();
        Self { dispositivos, indice }
    }

    pub fn get(&self, b: &Bdf) -> Option<&Dispositivo> {
        self.indice.get(b).map(|&i| &self.dispositivos[i])
    }

    /// Cadena de ancestros (del padre hacia arriba), con protección ante ciclos.
    pub fn ancestros(&self, b: &Bdf) -> Vec<Bdf> {
        let mut v = vec![];
        let mut actual = self.get(b).and_then(|d| d.padre.clone());
        while let Some(p) = actual {
            if v.contains(&p) || v.len() > 64 {
                break;
            }
            actual = self.get(&p).and_then(|d| d.padre.clone());
            v.push(p);
        }
        v
    }

    pub fn gpus(&self) -> Vec<&Dispositivo> {
        self.dispositivos.iter().filter(|d| d.tipo == Tipo::Gpu).collect()
    }

    pub fn relacion(&self, a: &Bdf, b: &Bdf) -> Relacion {
        if a == b {
            return Relacion::Mismo;
        }
        let (da, db) = match (self.get(a), self.get(b)) {
            (Some(x), Some(y)) => (x, y),
            _ => return Relacion::Sys,
        };
        if da.raiz != db.raiz {
            return if da.numa.is_some() && da.numa == db.numa { Relacion::Node } else { Relacion::Sys };
        }
        let (aa, ab) = (self.ancestros(a), self.ancestros(b));
        // Primer ancestro común (incluyendo que uno sea ancestro del otro).
        let comun = aa.iter().position(|x| ab.contains(x) || x == b).map(|i| (i, &aa[i]));
        match comun {
            None => Relacion::Phb,
            Some((ia, lca)) => {
                let ib = ab.iter().position(|x| x == lca).unwrap_or(0);
                // ia/ib = saltos desde cada dispositivo hasta el LCA (0 = su padre).
                if ia == 0 && ib == 0 {
                    Relacion::Pix
                } else {
                    Relacion::Pxb
                }
            }
        }
    }

    /// Raíces del árbol (sin padre o con padre desconocido), agrupadas por root complex.
    pub fn arbol(&self) -> Vec<(usize, &Dispositivo)> {
        let mut hijos: HashMap<Option<&Bdf>, Vec<&Dispositivo>> = HashMap::new();
        for d in &self.dispositivos {
            let p = d.padre.as_ref().filter(|p| self.indice.contains_key(*p));
            hijos.entry(p).or_default().push(d);
        }
        let mut out = vec![];
        fn rec<'a>(d: &'a Dispositivo, nivel: usize, hijos: &HashMap<Option<&Bdf>, Vec<&'a Dispositivo>>, out: &mut Vec<(usize, &'a Dispositivo)>) {
            out.push((nivel, d));
            if nivel > 64 {
                return;
            }
            for h in hijos.get(&Some(&d.bdf)).into_iter().flatten() {
                rec(h, nivel + 1, hijos, out);
            }
        }
        let mut raices = hijos.get(&None).cloned().unwrap_or_default();
        raices.sort_by(|a, b| (&a.raiz, &a.bdf).cmp(&(&b.raiz, &b.bdf)));
        for r in raices {
            rec(r, 0, &hijos, &mut out);
        }
        out
    }

    /// Cuello de botella hasta el root complex: el enlace más lento de la cadena.
    pub fn cuello_de_botella(&self, b: &Bdf) -> Option<f32> {
        std::iter::once(b.clone())
            .chain(self.ancestros(b))
            .filter_map(|x| self.get(&x).and_then(|d| d.actual).map(|e| e.gb_s()))
            .min_by(f32::total_cmp)
    }

    /// Matriz de texto GPU×GPU.
    pub fn matriz_texto(&self) -> String {
        let g = self.gpus();
        let mut s = format!("{:<8}", "");
        for i in 0..g.len() {
            s += &format!("{:<6}", format!("GPU{i}"));
        }
        s += "NUMA\n";
        for (i, a) in g.iter().enumerate() {
            s += &format!("{:<8}", format!("GPU{i}"));
            for b in &g {
                s += &format!("{:<6}", self.relacion(&a.bdf, &b.bdf).sigla());
            }
            s += &format!("{}\n", a.numa.map(|n| n.to_string()).unwrap_or("-".into()));
        }
        s
    }
}
