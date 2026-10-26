//! Autómatas finitos: AFD, AFN‑ε, construcción de subconjuntos y minimización.
//!
//! Los estados se numeran `0..n`, lo que hace que todo sea más rápido y
//! simple que usar nombres. Las transiciones ausentes de un AFD van a un
//! estado "sumidero" implícito, que rechaza.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

/// Autómata finito determinista.
#[derive(Debug, Clone)]
pub struct Afd {
    /// Número de estados.
    pub n: usize,
    /// Estado inicial.
    pub inicial: usize,
    /// Estados de aceptación.
    pub aceptacion: HashSet<usize>,
    /// δ(estado, símbolo) → estado. Si falta, se rechaza.
    pub delta: HashMap<(usize, char), usize>,
}

impl Afd {
    /// ¿Acepta la cadena? O(|w|): una sola pasada, memoria constante.
    pub fn acepta(&self, w: &str) -> bool {
        let mut q = self.inicial;
        for c in w.chars() {
            match self.delta.get(&(q, c)) {
                Some(&s) => q = s,
                None => return false, // sumidero implícito
            }
        }
        self.aceptacion.contains(&q)
    }

    /// Alfabeto usado por δ (ordenado, para resultados deterministas).
    pub fn alfabeto(&self) -> Vec<char> {
        let s: BTreeSet<char> = self.delta.keys().map(|&(_, c)| c).collect();
        s.into_iter().collect()
    }

    /// AFD de los números binarios divisibles por `k`. El estado es el
    /// resto actual: al leer el bit b, el nuevo resto es (2·r + b) mod k.
    pub fn multiplo_de(k: usize) -> Afd {
        let mut delta = HashMap::new();
        for r in 0..k {
            delta.insert((r, '0'), (2 * r) % k);
            delta.insert((r, '1'), (2 * r + 1) % k);
        }
        Afd { n: k, inicial: 0, aceptacion: [0].into(), delta }
    }

    /// **Minimización** por refinamiento de particiones (algoritmo de Moore).
    ///
    /// 1. Eliminar los estados inalcanzables.
    /// 2. Completar con un sumidero explícito, para que δ sea total.
    /// 3. Partir en {aceptación, resto} y refinar: dos estados siguen
    ///    juntos solo si, para cada símbolo, van al mismo bloque. Se repite
    ///    hasta que no cambie nada. Cada bloque final es un estado del AFD mínimo.
    pub fn minimizar(&self) -> Afd {
        let alf = self.alfabeto();
        // 1. Alcanzables (BFS).
        let mut alcanzables = vec![self.inicial];
        let mut visto: HashSet<usize> = [self.inicial].into();
        let mut i = 0;
        while i < alcanzables.len() {
            let q = alcanzables[i];
            for &c in &alf {
                if let Some(&s) = self.delta.get(&(q, c)) {
                    if visto.insert(s) {
                        alcanzables.push(s);
                    }
                }
            }
            i += 1;
        }
        // 2. Renumerar y añadir sumidero (índice m).
        let m = alcanzables.len();
        let idx: HashMap<usize, usize> = alcanzables.iter().enumerate().map(|(i, &q)| (q, i)).collect();
        let total = |q: usize, c: char| -> usize {
            if q == m {
                return m;
            }
            self.delta.get(&(alcanzables[q], c)).map_or(m, |s| idx[s])
        };
        let acepta = |q: usize| q < m && self.aceptacion.contains(&alcanzables[q]);
        // 3. Refinamiento. `bloque[q]` = identificador de bloque de q.
        let mut bloque: Vec<usize> = (0..=m).map(|q| usize::from(acepta(q))).collect();
        loop {
            // Firma de cada estado: su bloque actual + bloques destino.
            let mut firmas: HashMap<Vec<usize>, usize> = HashMap::new();
            let nuevo: Vec<usize> = (0..=m)
                .map(|q| {
                    let mut f = vec![bloque[q]];
                    f.extend(alf.iter().map(|&c| bloque[total(q, c)]));
                    let n = firmas.len();
                    *firmas.entry(f).or_insert(n)
                })
                .collect();
            let n_bloques_ant = bloque.iter().collect::<HashSet<_>>().len();
            let estable = firmas.len() == n_bloques_ant;
            bloque = nuevo;
            if estable {
                break;
            }
        }
        // Construcción del AFD cociente. Los identificadores de bloque no
        // son consecutivos, así que los renumeramos 0..n. El bloque del
        // sumidero (estados "muertos", desde los que no se acepta nunca) se
        // descarta y vuelve a ser implícito, salvo que contenga el estado
        // inicial (lenguaje vacío): entonces queda un único estado.
        let sumidero = bloque[m];
        let mut nuevo_id: HashMap<usize, usize> = HashMap::new();
        let mut id = |b: usize| {
            let n = nuevo_id.len();
            *nuevo_id.entry(b).or_insert(n)
        };
        let inicial = id(bloque[0]); // el inicial siempre es el 0
        let mut delta = HashMap::new();
        let mut aceptacion = HashSet::new();
        for q in 0..m {
            if bloque[q] == sumidero && q != 0 {
                continue;
            }
            let bq = id(bloque[q]);
            if acepta(q) {
                aceptacion.insert(bq);
            }
            for &c in &alf {
                let d = bloque[total(q, c)];
                if d != sumidero {
                    let bd = id(d);
                    delta.insert((bq, c), bd);
                }
            }
        }
        Afd { n: nuevo_id.len(), inicial, aceptacion, delta }
    }
}

/// AFN con transiciones ε (representadas con `None`).
#[derive(Debug, Clone, Default)]
pub struct Afn {
    /// Número de estados.
    pub n: usize,
    /// Estado inicial.
    pub inicial: usize,
    /// Estados de aceptación.
    pub aceptacion: HashSet<usize>,
    /// δ(estado, símbolo o ε) → conjunto de estados.
    pub delta: HashMap<(usize, Option<char>), Vec<usize>>,
}

impl Afn {
    /// Añade una transición (`None` = ε).
    pub fn transicion(&mut self, de: usize, c: Option<char>, a: usize) {
        self.delta.entry((de, c)).or_default().push(a);
        self.n = self.n.max(de + 1).max(a + 1);
    }

    /// Clausura‑ε: todos los estados alcanzables desde `conj` usando solo ε.
    pub fn clausura(&self, conj: &BTreeSet<usize>) -> BTreeSet<usize> {
        let mut res = conj.clone();
        let mut pila: Vec<usize> = conj.iter().copied().collect();
        while let Some(q) = pila.pop() {
            for &s in self.delta.get(&(q, None)).into_iter().flatten() {
                if res.insert(s) {
                    pila.push(s);
                }
            }
        }
        res
    }

    /// Mueve el conjunto con el símbolo `c` (sin clausura).
    fn mover(&self, conj: &BTreeSet<usize>, c: char) -> BTreeSet<usize> {
        conj.iter()
            .flat_map(|&q| self.delta.get(&(q, Some(c))).into_iter().flatten().copied())
            .collect()
    }

    /// Simulación directa: se mantiene el **conjunto** de estados activos.
    /// O(|w|·n²) como máximo, pero sin construir el AFD.
    pub fn acepta(&self, w: &str) -> bool {
        let mut actual = self.clausura(&[self.inicial].into());
        for c in w.chars() {
            actual = self.clausura(&self.mover(&actual, c));
            if actual.is_empty() {
                return false;
            }
        }
        actual.iter().any(|q| self.aceptacion.contains(q))
    }

    /// **Construcción de subconjuntos**: cada estado del AFD es un conjunto
    /// de estados del AFN. Solo se generan los alcanzables (BFS), así que
    /// en la práctica rara vez se llega a los 2ⁿ teóricos.
    pub fn a_afd(&self) -> Afd {
        let alf: BTreeSet<char> = self.delta.keys().filter_map(|&(_, c)| c).collect();
        let ini = self.clausura(&[self.inicial].into());
        let mut ids: HashMap<BTreeSet<usize>, usize> = HashMap::from([(ini.clone(), 0)]);
        let mut cola = VecDeque::from([ini]);
        let mut afd = Afd { n: 0, inicial: 0, aceptacion: HashSet::new(), delta: HashMap::new() };
        while let Some(conj) = cola.pop_front() {
            let id = ids[&conj];
            if conj.iter().any(|q| self.aceptacion.contains(q)) {
                afd.aceptacion.insert(id);
            }
            for &c in &alf {
                let sig = self.clausura(&self.mover(&conj, c));
                if sig.is_empty() {
                    continue; // sumidero implícito
                }
                let n = ids.len();
                let sid = *ids.entry(sig.clone()).or_insert_with(|| {
                    cola.push_back(sig);
                    n
                });
                afd.delta.insert((id, c), sid);
            }
        }
        afd.n = ids.len();
        afd
    }

    /// AFN de "cadenas sobre {a,b} cuyo k‑ésimo símbolo desde el final es 'a'".
    /// Tiene k+1 estados, pero su AFD mínimo necesita 2ᵏ: es el ejemplo
    /// clásico de explosión exponencial.
    pub fn kesimo_desde_final(k: usize) -> Afn {
        let mut a = Afn::default();
        a.transicion(0, Some('a'), 0);
        a.transicion(0, Some('b'), 0);
        a.transicion(0, Some('a'), 1);
        for i in 1..k {
            a.transicion(i, Some('a'), i + 1);
            a.transicion(i, Some('b'), i + 1);
        }
        a.aceptacion.insert(k);
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplo_de_3() {
        let d = Afd::multiplo_de(3);
        for n in 0u32..200 {
            assert_eq!(d.acepta(&format!("{n:b}")), n % 3 == 0, "{n}");
        }
    }

    #[test]
    fn clausura_epsilon_transitiva() {
        let mut a = Afn::default();
        a.transicion(0, None, 1);
        a.transicion(1, None, 2);
        a.transicion(2, Some('x'), 3);
        assert_eq!(a.clausura(&[0].into()), [0, 1, 2].into());
    }

    #[test]
    fn explosion_exponencial() {
        for k in 1..=6 {
            let afn = Afn::kesimo_desde_final(k);
            let min = afn.a_afd().minimizar();
            assert_eq!(min.n, 1 << k, "k={k}");
        }
    }

    #[test]
    fn minimizar_fusiona_equivalentes() {
        // AFD de "termina en 1" con estados redundantes (0≡2, 1≡3).
        let mut delta = HashMap::new();
        for (q, c, s) in [(0, '0', 2), (0, '1', 1), (1, '0', 2), (1, '1', 3), (2, '0', 0), (2, '1', 3), (3, '0', 0), (3, '1', 1)] {
            delta.insert((q, c), s);
        }
        let d = Afd { n: 4, inicial: 0, aceptacion: [1, 3].into(), delta };
        let m = d.minimizar();
        assert_eq!(m.n, 2);
        for w in ["", "1", "10", "0101", "111", "1110"] {
            assert_eq!(m.acepta(w), d.acepta(w), "{w}");
        }
    }
}
