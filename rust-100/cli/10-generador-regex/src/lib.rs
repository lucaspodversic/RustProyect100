//! # Generador de regex — algoritmo
//!
//! Dado un conjunto de ejemplos, genera una regex anclada (`^...$`) que acepta
//! **exactamente** esos ejemplos (o una generalización controlada si se piden
//! clases como `\d`). Pasos:
//!
//! 1. **Tokenización** ([`tokenizar`]): cada carácter pasa a un [`Clase`]
//!    (literal, o `\d`/`\w`/`\s` si se activó la generalización) y las
//!    repeticiones consecutivas se comprimen: `aaa` → `a{3}` (si `repeticiones`).
//! 2. **Trie** ([`Trie`]): se insertan las secuencias de tokens; los prefijos
//!    comunes quedan factorizados automáticamente (`abc|abd` → `ab[cd]`).
//! 3. **Render** ([`Trie::render`]): recorrido recursivo que emite alternancias
//!    `(?:x|y)`, convierte alternativas de un solo carácter en clases `[xy]` y
//!    marca como opcional `(?:...)?` un sufijo cuando un ejemplo termina antes.
//! 4. **Verificación**: [`generar`] compila la regex y comprueba que acepte
//!    todos los ejemplos (si no, es un bug y se devuelve error).

use regex::Regex;
use std::collections::BTreeMap;

/// Clase de un carácter según las opciones de generalización.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Clase {
    Literal(char),
    Digito,
    Palabra,
    Espacio,
}

/// Token = clase + cantidad de repeticiones consecutivas.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token {
    pub clase: Clase,
    pub veces: usize,
}

/// Opciones de generación.
#[derive(Debug, Clone, Copy, Default)]
pub struct Opciones {
    /// Convierte dígitos a `\d`.
    pub digitos: bool,
    /// Convierte letras/dígitos/_ a `\w` (tiene prioridad sobre `digitos`).
    pub palabras: bool,
    /// Convierte espacios en blanco a `\s`.
    pub espacios: bool,
    /// Comprime repeticiones (`aaa` → `a{3}`).
    pub repeticiones: bool,
    /// Agrega el flag `(?i)` (no distingue mayúsculas): los ejemplos se pasan a minúsculas.
    pub ignorar_caso: bool,
}

/// Escapa un carácter si es metacarácter de regex.
fn escapar(c: char) -> String {
    if "\\.+*?()|[]{}^$#&-~".contains(c) {
        format!("\\{c}")
    } else if c.is_whitespace() && c != ' ' {
        // Tabs, saltos, etc.: los escribimos como escape Unicode para que se vean.
        format!("\\u{{{:x}}}", c as u32)
    } else {
        c.to_string()
    }
}

impl Clase {
    fn texto(&self) -> String {
        match self {
            Clase::Literal(c) => escapar(*c),
            Clase::Digito => "\\d".into(),
            Clase::Palabra => "\\w".into(),
            Clase::Espacio => "\\s".into(),
        }
    }
}

impl Token {
    fn texto(&self) -> String {
        let base = self.clase.texto();
        match self.veces {
            1 => base,
            n => format!("{base}{{{n}}}"),
        }
    }
    /// ¿Se puede meter dentro de `[...]`? Solo un carácter sin repetición.
    fn es_simple(&self) -> bool {
        self.veces == 1
    }
}

/// Convierte un ejemplo en tokens según las opciones.
pub fn tokenizar(s: &str, o: &Opciones) -> Vec<Token> {
    let mut v: Vec<Token> = Vec::new();
    for c in s.chars() {
        let clase = if o.palabras && (c.is_alphanumeric() || c == '_') {
            Clase::Palabra
        } else if o.digitos && c.is_ascii_digit() {
            Clase::Digito
        } else if o.espacios && c.is_whitespace() {
            Clase::Espacio
        } else {
            Clase::Literal(c)
        };
        // Si se comprimen repeticiones y coincide con el anterior, sumamos.
        match v.last_mut() {
            Some(t) if o.repeticiones && t.clase == clase => t.veces += 1,
            _ => v.push(Token { clase, veces: 1 }),
        }
    }
    v
}

/// Trie de tokens. `BTreeMap` → hijos ordenados → salida determinista.
#[derive(Debug, Default)]
pub struct Trie {
    hijos: BTreeMap<Token, Trie>,
    /// Algún ejemplo termina en este nodo.
    terminal: bool,
}

impl Trie {
    pub fn insertar(&mut self, tokens: &[Token]) {
        let mut n = self;
        for t in tokens {
            n = n.hijos.entry(t.clone()).or_default();
        }
        n.terminal = true;
    }

    /// Render del subárbol que cuelga de este nodo (sin incluir el nodo).
    pub fn render(&self) -> String {
        if self.hijos.is_empty() {
            return String::new();
        }
        // Una rama por hijo: token + render del resto.
        let ramas: Vec<(Token, String)> = self.hijos.iter().map(|(t, h)| (t.clone(), h.render())).collect();
        // Si TODAS las ramas son "un carácter simple sin continuación", usamos clase [..].
        let cuerpo = if ramas.len() > 1 && ramas.iter().all(|(t, r)| r.is_empty() && t.es_simple()) {
            clase_de_caracteres(&ramas.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>())
        } else if ramas.len() == 1 {
            let (t, r) = &ramas[0];
            format!("{}{r}", t.texto())
        } else {
            let alts: Vec<String> = ramas.iter().map(|(t, r)| format!("{}{r}", t.texto())).collect();
            format!("(?:{})", alts.join("|"))
        };
        if self.terminal {
            // Un ejemplo termina acá: lo que sigue es opcional.
            if es_atomo(&cuerpo) {
                format!("{cuerpo}?")
            } else {
                format!("(?:{cuerpo})?")
            }
        } else {
            cuerpo
        }
    }
}

/// ¿El texto es un único átomo regex (al que se le puede poner `?` directo)?
fn es_atomo(s: &str) -> bool {
    let cs: Vec<char> = s.chars().collect();
    match cs.as_slice() {
        [_] => true,
        ['\\', _] => true,
        ['[', .., ']'] => !s[1..s.len() - 1].contains(']') || s.ends_with("\\]]"),
        ['(', '?', ':', .., ')'] => {
            // Comprobamos que el paréntesis inicial cierre al final (grupo único).
            let mut prof = 0i32;
            let mut esc = false;
            for (i, c) in cs.iter().enumerate() {
                if esc {
                    esc = false;
                    continue;
                }
                match c {
                    '\\' => esc = true,
                    '(' => prof += 1,
                    ')' => {
                        prof -= 1;
                        if prof == 0 && i != cs.len() - 1 {
                            return false;
                        }
                    }
                    _ => {}
                }
            }
            true
        }
        _ => false,
    }
}

/// Arma `[abc]` o rangos `[a-e]` cuando hay 3+ literales consecutivos.
fn clase_de_caracteres(ts: &[Token]) -> String {
    let mut lits: Vec<char> = Vec::new();
    let mut otros: Vec<String> = Vec::new();
    for t in ts {
        match t.clase {
            Clase::Literal(c) => lits.push(c),
            ref k => otros.push(k.texto()),
        }
    }
    lits.sort();
    lits.dedup();
    let mut s = String::from("[");
    s.push_str(&otros.join(""));
    let mut i = 0;
    while i < lits.len() {
        // Buscamos una corrida consecutiva a..=e
        let mut j = i;
        while j + 1 < lits.len() && (lits[j + 1] as u32) == (lits[j] as u32) + 1 && lits[j].is_alphanumeric() && lits[j + 1].is_alphanumeric() {
            j += 1;
        }
        if j - i >= 2 {
            s.push_str(&format!("{}-{}", escapar_en_clase(lits[i]), escapar_en_clase(lits[j])));
        } else {
            for c in &lits[i..=j] {
                s.push_str(&escapar_en_clase(*c));
            }
        }
        i = j + 1;
    }
    s.push(']');
    s
}

/// Dentro de `[...]` solo hay que escapar `\ ] [ ^ -`.
fn escapar_en_clase(c: char) -> String {
    if "\\]-^[".contains(c) {
        format!("\\{c}")
    } else {
        escapar_unicode(c)
    }
}

/// Caracteres de espacio no visibles (tab, salto) se escriben como `\u{..}`.
fn escapar_unicode(c: char) -> String {
    if c.is_whitespace() && c != ' ' { format!("\\u{{{:x}}}", c as u32) } else { c.to_string() }
}

/// Genera la regex y verifica que acepte todos los ejemplos.
pub fn generar(ejemplos: &[String], o: &Opciones) -> Result<String, String> {
    if ejemplos.is_empty() {
        return Err("se necesita al menos un ejemplo".into());
    }
    let mut trie = Trie::default();
    for e in ejemplos {
        let e = if o.ignorar_caso { e.to_lowercase() } else { e.clone() };
        trie.insertar(&tokenizar(&e, o));
    }
    let flag = if o.ignorar_caso { "(?i)" } else { "" };
    let re = format!("{flag}^{}$", trie.render());
    // Verificación de seguridad: si la regex no acepta algún ejemplo, es un bug.
    let compilada = Regex::new(&re).map_err(|e| format!("regex generada inválida ({re}): {e}"))?;
    if let Some(m) = ejemplos.iter().find(|e| !compilada.is_match(e)) {
        return Err(format!("bug: la regex {re} no acepta el ejemplo {m:?}"));
    }
    Ok(re)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| x.to_string()).collect()
    }
    fn g(xs: &[&str], o: Opciones) -> String {
        generar(&s(xs), &o).unwrap()
    }

    #[test]
    fn un_ejemplo_escapado() {
        assert_eq!(g(&["a.b"], Opciones::default()), r"^a\.b$");
    }

    #[test]
    fn prefijo_comun_y_clase() {
        assert_eq!(g(&["abc", "abd"], Opciones::default()), "^ab[cd]$");
    }

    #[test]
    fn alternancia_y_opcional() {
        assert_eq!(g(&["hola", "holanda"], Opciones::default()), "^hola(?:nda)?$");
        assert_eq!(g(&["perro", "gato"], Opciones::default()), "^(?:gato|perro)$");
        assert_eq!(g(&["a", "ab"], Opciones::default()), "^ab?$");
    }

    #[test]
    fn rangos() {
        assert_eq!(g(&["a", "b", "c", "e"], Opciones::default()), "^[a-ce]$");
    }

    #[test]
    fn digitos_y_repeticiones() {
        let o = Opciones { digitos: true, repeticiones: true, ..Default::default() };
        assert_eq!(g(&["2024-01-05", "1999-12-31"], o), r"^\d{4}\-\d{2}\-\d{2}$");
        let r = Regex::new(&g(&["123"], o)).unwrap();
        assert!(r.is_match("987") && !r.is_match("12"));
    }

    #[test]
    fn exactitud_sin_generalizar() {
        let ej = ["foo", "foobar", "fizz", "buzz", "f.o"];
        let re = Regex::new(&g(&ej, Opciones::default())).unwrap();
        for e in ej {
            assert!(re.is_match(e), "{e}");
        }
        for no in ["fo", "fooba", "fizzbuzz", "fxo", ""] {
            assert!(!re.is_match(no), "{no} no debería coincidir");
        }
    }

    #[test]
    fn ignorar_caso_y_espacios() {
        let o = Opciones { ignorar_caso: true, espacios: true, ..Default::default() };
        let re = g(&["Hola Mundo"], o);
        assert_eq!(re, r"(?i)^hola\smundo$");
        assert!(Regex::new(&re).unwrap().is_match("HOLA\tmundo"));
    }

    #[test]
    fn caracteres_especiales_en_clase() {
        let re = g(&["-", "]", "a"], Opciones::default());
        let r = Regex::new(&re).unwrap();
        assert!(r.is_match("-") && r.is_match("]") && r.is_match("a") && !r.is_match("b"));
    }

    #[test]
    fn sin_ejemplos_es_error() {
        assert!(generar(&[], &Opciones::default()).is_err());
    }
}
