//! # Mini Search Engine
//!
//! Índice invertido con ranking **TF-IDF** + coseno.
//! - Tokenización simple (minúsculas, alfanumérico).
//! - Índice: término → lista de (doc_id, tf).
//! - IDF = ln(N / df) (suavizado +1 en df).
//! - Score = coseno entre el vector de la query y el del documento.

use std::collections::{HashMap, HashSet};

/// Un documento indexado.
#[derive(Debug, Clone)]
pub struct Document {
    pub id: String,
    pub text: String,
}

/// Tokeniza: minúsculas y trozos alfanuméricos (Unicode).
pub fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            cur.extend(ch.to_lowercase());
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[derive(Debug, Default)]
pub struct Index {
    /// término → (doc_id → tf)
    postings: HashMap<String, HashMap<String, f64>>,
    /// doc_id → cantidad de tokens (para normalizar TF)
    doc_len: HashMap<String, usize>,
    /// doc_id → texto original (para snippet)
    docs: HashMap<String, String>,
}

impl Index {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, doc: Document) {
        let tokens = tokenize(&doc.text);
        self.doc_len.insert(doc.id.clone(), tokens.len());
        let mut tf: HashMap<String, f64> = HashMap::new();
        for t in &tokens {
            *tf.entry(t.clone()).or_default() += 1.0;
        }
        for (term, c) in tf {
            self.postings.entry(term).or_default().insert(doc.id.clone(), c);
        }
        self.docs.insert(doc.id, doc.text);
    }

    fn idf(&self, term: &str) -> f64 {
        let n = self.docs.len().max(1) as f64;
        let df = self.postings.get(term).map(|p| p.len()).unwrap_or(0) as f64;
        (n / (1.0 + df)).ln() + 1.0 // +1 evita idf=0 y suaviza
    }

    /// Vector TF-IDF de un documento (solo términos presentes).
    fn doc_vec(&self, id: &str) -> HashMap<String, f64> {
        let len = *self.doc_len.get(id).unwrap_or(&1) as f64;
        let mut v = HashMap::new();
        for (term, posting) in &self.postings {
            if let Some(&tf) = posting.get(id) {
                v.insert(term.clone(), (tf / len) * self.idf(term));
            }
        }
        v
    }

    fn query_vec(&self, q: &str) -> HashMap<String, f64> {
        let tokens = tokenize(q);
        let mut tf: HashMap<String, f64> = HashMap::new();
        for t in &tokens {
            *tf.entry(t.clone()).or_default() += 1.0;
        }
        let len = tokens.len().max(1) as f64;
        tf.into_iter().map(|(t, c)| (t.clone(), (c / len) * self.idf(&t))).collect()
    }

    fn cosine(a: &HashMap<String, f64>, b: &HashMap<String, f64>) -> f64 {
        let mut dot = 0.0;
        for (k, va) in a {
            if let Some(vb) = b.get(k) {
                dot += va * vb;
            }
        }
        let na: f64 = a.values().map(|x| x * x).sum::<f64>().sqrt();
        let nb: f64 = b.values().map(|x| x * x).sum::<f64>().sqrt();
        if na == 0.0 || nb == 0.0 {
            0.0
        } else {
            dot / (na * nb)
        }
    }

    /// Busca y devuelve (doc_id, score, snippet) ordenado desc.
    pub fn search(&self, query: &str, k: usize) -> Vec<(String, f64, String)> {
        let qv = self.query_vec(query);
        let mut candidates: HashSet<String> = HashSet::new();
        for term in qv.keys() {
            if let Some(p) = self.postings.get(term) {
                candidates.extend(p.keys().cloned());
            }
        }
        let mut scored: Vec<_> = candidates
            .into_iter()
            .map(|id| {
                let score = Self::cosine(&qv, &self.doc_vec(&id));
                let snip = self.docs.get(&id).cloned().unwrap_or_default();
                let snip: String = snip.chars().take(80).collect();
                (id, score, snip)
            })
            .filter(|(_, s, _)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(k);
        scored
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranking_tfidf() {
        let mut ix = Index::new();
        ix.add(Document { id: "a".into(), text: "el gato come pescado".into() });
        ix.add(Document { id: "b".into(), text: "el perro come carne".into() });
        ix.add(Document { id: "c".into(), text: "pescado fresco del mar".into() });
        let r = ix.search("gato pescado", 3);
        assert_eq!(r[0].0, "a", "a menciona ambos términos");
        assert!(r.iter().any(|(id, _, _)| id == "c"));
        assert!(!r.iter().any(|(id, _, _)| id == "b") || r.iter().find(|(id,_,_)| id=="b").unwrap().1 < r[0].1);
    }

    #[test]
    fn tokenize_unicode() {
        assert_eq!(tokenize("¡Hola, Niño!"), vec!["hola", "niño"]);
    }

    #[test]
    fn vacio() {
        let ix = Index::new();
        assert!(ix.search("nada", 5).is_empty());
    }
}
