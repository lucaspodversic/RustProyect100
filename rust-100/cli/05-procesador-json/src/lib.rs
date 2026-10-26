//! # Procesador JSON — lógica (un "mini-jq")
//!
//! Tres piezas:
//! - **Rutas** ([`Ruta`], [`parsear_ruta`]): sintaxis `.a.b[0].c`, `.items[]`
//!   (iterar), `."clave con espacios"`.
//! - **Operaciones**: [`extraer`] (get), [`filtrar`] (where), [`elegir`] (pick
//!   de campos), [`asignar`] (set) y [`borrar`] (del).
//! - **Condiciones** ([`Condicion`]): `campo op valor`, con `== != > >= < <= ~`
//!   (`~` = contiene, para strings y arrays).

use serde_json::{Map, Value};
use std::cmp::Ordering;

/// Un segmento de ruta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segmento {
    /// Acceso a una clave de objeto.
    Clave(String),
    /// Acceso por índice a un array (admite negativos: -1 = último).
    Indice(i64),
    /// `[]`: itera todos los elementos (fan-out).
    Todos,
}

/// Ruta completa = secuencia de segmentos. `.` sola es la raíz (vacía).
pub type Ruta = Vec<Segmento>;

/// Errores del procesador con mensajes en español.
#[derive(Debug, PartialEq, Eq)]
pub struct ErrorJson(pub String);

impl std::fmt::Display for ErrorJson {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ErrorJson {}

fn err<T>(m: impl Into<String>) -> Result<T, ErrorJson> {
    Err(ErrorJson(m.into()))
}

/// Parser escrito a mano (recursive-descent lineal) para la sintaxis de rutas.
pub fn parsear_ruta(s: &str) -> Result<Ruta, ErrorJson> {
    let c: Vec<char> = s.trim().chars().collect();
    let mut i = 0;
    let mut r = Vec::new();
    if c.is_empty() {
        return Ok(r);
    }
    while i < c.len() {
        match c[i] {
            '.' => {
                i += 1;
                if i >= c.len() {
                    break; // "." sola o punto final
                }
                if c[i] == '"' {
                    // Clave entre comillas: ."con espacios"
                    i += 1;
                    let ini = i;
                    while i < c.len() && c[i] != '"' {
                        i += 1;
                    }
                    if i >= c.len() {
                        return err("comilla sin cerrar en la ruta");
                    }
                    r.push(Segmento::Clave(c[ini..i].iter().collect()));
                    i += 1;
                } else if c[i] == '[' {
                    continue; // ".[0]" equivale a "[0]"
                } else {
                    let ini = i;
                    while i < c.len() && c[i] != '.' && c[i] != '[' {
                        i += 1;
                    }
                    r.push(Segmento::Clave(c[ini..i].iter().collect()));
                }
            }
            '[' => {
                i += 1;
                let ini = i;
                while i < c.len() && c[i] != ']' {
                    i += 1;
                }
                if i >= c.len() {
                    return err("corchete sin cerrar en la ruta");
                }
                let dentro: String = c[ini..i].iter().collect();
                i += 1;
                if dentro.trim().is_empty() {
                    r.push(Segmento::Todos);
                } else {
                    let n: i64 = dentro.trim().parse().map_err(|_| ErrorJson(format!("índice inválido: {dentro}")))?;
                    r.push(Segmento::Indice(n));
                }
            }
            otro => return err(format!("carácter inesperado '{otro}' en la ruta (¿falta el punto inicial?)")),
        }
    }
    Ok(r)
}

/// Normaliza un índice posiblemente negativo contra la longitud.
fn idx(n: i64, len: usize) -> Option<usize> {
    let i = if n < 0 { len as i64 + n } else { n };
    (0..len as i64).contains(&i).then_some(i as usize)
}

/// Extrae todos los valores que coinciden con la ruta.
/// Con `[]` puede devolver muchos; si la ruta no existe, devuelve vacío.
pub fn extraer<'a>(v: &'a Value, ruta: &[Segmento]) -> Vec<&'a Value> {
    let Some((primero, resto)) = ruta.split_first() else { return vec![v] };
    match (primero, v) {
        (Segmento::Clave(k), Value::Object(m)) => m.get(k).map(|x| extraer(x, resto)).unwrap_or_default(),
        (Segmento::Indice(n), Value::Array(a)) => idx(*n, a.len()).map(|i| extraer(&a[i], resto)).unwrap_or_default(),
        (Segmento::Todos, Value::Array(a)) => a.iter().flat_map(|x| extraer(x, resto)).collect(),
        (Segmento::Todos, Value::Object(m)) => m.values().flat_map(|x| extraer(x, resto)).collect(),
        _ => vec![],
    }
}

/// Operadores de comparación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Igual,
    Distinto,
    Mayor,
    MayorIgual,
    Menor,
    MenorIgual,
    Contiene,
}

/// Condición `ruta op valor` para filtrar elementos de un array.
#[derive(Debug, Clone, PartialEq)]
pub struct Condicion {
    pub ruta: Ruta,
    pub op: Op,
    pub valor: Value,
}

/// Parsea `".edad >= 30"` o `".nombre ~ \"an\""`. El valor se interpreta como
/// JSON; si no es JSON válido, se toma como string literal (comodidad en la shell).
pub fn parsear_condicion(s: &str) -> Result<Condicion, ErrorJson> {
    // Probamos los operadores de 2 caracteres antes que los de 1.
    const OPS: [(&str, Op); 7] = [
        ("==", Op::Igual),
        ("!=", Op::Distinto),
        (">=", Op::MayorIgual),
        ("<=", Op::MenorIgual),
        (">", Op::Mayor),
        ("<", Op::Menor),
        ("~", Op::Contiene),
    ];
    for (t, op) in OPS {
        if let Some(p) = s.find(t) {
            let ruta = parsear_ruta(s[..p].trim())?;
            let crudo = s[p + t.len()..].trim();
            let valor = serde_json::from_str(crudo).unwrap_or_else(|_| Value::String(crudo.to_string()));
            return Ok(Condicion { ruta, op, valor });
        }
    }
    err(format!("condición sin operador: {s}"))
}

/// Compara dos valores JSON: números con números, strings con strings.
fn comparar(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64()?.partial_cmp(&y.as_f64()?),
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

impl Condicion {
    /// Evalúa la condición contra un elemento. Si la ruta no existe → `false`
    /// (salvo `!=`, que es `true`: "no es igual" a algo inexistente).
    pub fn evaluar(&self, elem: &Value) -> bool {
        let vals = extraer(elem, &self.ruta);
        let Some(x) = vals.first() else { return self.op == Op::Distinto };
        match self.op {
            Op::Igual => *x == &self.valor,
            Op::Distinto => *x != &self.valor,
            Op::Mayor => comparar(x, &self.valor) == Some(Ordering::Greater),
            Op::MayorIgual => matches!(comparar(x, &self.valor), Some(Ordering::Greater | Ordering::Equal)),
            Op::Menor => comparar(x, &self.valor) == Some(Ordering::Less),
            Op::MenorIgual => matches!(comparar(x, &self.valor), Some(Ordering::Less | Ordering::Equal)),
            Op::Contiene => match (x, &self.valor) {
                (Value::String(s), Value::String(t)) => s.contains(t.as_str()),
                (Value::Array(a), v) => a.contains(v),
                _ => false,
            },
        }
    }
}

/// Filtra un array por una condición. Error si `v` no es un array.
pub fn filtrar(v: &Value, c: &Condicion) -> Result<Value, ErrorJson> {
    match v {
        Value::Array(a) => Ok(Value::Array(a.iter().filter(|e| c.evaluar(e)).cloned().collect())),
        _ => err("filtrar requiere un array (usá --ruta para llegar a uno)"),
    }
}

/// Proyección: deja solo los campos indicados en un objeto (o en cada objeto de un array).
pub fn elegir(v: &Value, campos: &[String]) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().map(|e| elegir(e, campos)).collect()),
        Value::Object(m) => {
            let mut out = Map::new();
            for c in campos {
                if let Some(x) = m.get(c) {
                    out.insert(c.clone(), x.clone());
                }
            }
            Value::Object(out)
        }
        otro => otro.clone(),
    }
}

/// Asigna `nuevo` en la ruta, creando objetos intermedios si faltan.
/// No soporta `[]` (sería ambiguo) ni crear índices inexistentes.
pub fn asignar(v: &mut Value, ruta: &[Segmento], nuevo: Value) -> Result<(), ErrorJson> {
    let Some((primero, resto)) = ruta.split_first() else {
        *v = nuevo;
        return Ok(());
    };
    match primero {
        Segmento::Clave(k) => {
            if v.is_null() {
                *v = Value::Object(Map::new()); // auto-creación de intermedios
            }
            let Value::Object(m) = v else { return err(format!("'{k}': no es un objeto")) };
            asignar(m.entry(k.clone()).or_insert(Value::Null), resto, nuevo)
        }
        Segmento::Indice(n) => {
            let Value::Array(a) = v else { return err("índice sobre algo que no es array") };
            let len = a.len();
            let i = idx(*n, len).ok_or_else(|| ErrorJson(format!("índice {n} fuera de rango (len {len})")))?;
            asignar(&mut a[i], resto, nuevo)
        }
        Segmento::Todos => err("no se puede asignar con []"),
    }
}

/// Borra la clave/índice final de la ruta. Devuelve `true` si borró algo.
pub fn borrar(v: &mut Value, ruta: &[Segmento]) -> bool {
    match ruta {
        [] => false,
        [ultimo] => match (ultimo, v) {
            (Segmento::Clave(k), Value::Object(m)) => m.shift_remove(k).is_some(),
            (Segmento::Indice(n), Value::Array(a)) => match idx(*n, a.len()) {
                Some(i) => {
                    a.remove(i);
                    true
                }
                None => false,
            },
            _ => false,
        },
        [primero, resto @ ..] => match (primero, v) {
            (Segmento::Clave(k), Value::Object(m)) => m.get_mut(k).is_some_and(|x| borrar(x, resto)),
            (Segmento::Indice(n), Value::Array(a)) => {
                let len = a.len();
                idx(*n, len).is_some_and(|i| borrar(&mut a[i], resto))
            }
            _ => false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn datos() -> Value {
        json!({"usuarios":[
            {"nombre":"Ana","edad":31,"tags":["admin"]},
            {"nombre":"Beto","edad":25,"tags":[]},
            {"nombre":"Caro","edad":40,"tags":["dev","admin"]}
        ], "meta": {"total en lista": 3}})
    }

    #[test]
    fn parsea_rutas() {
        assert_eq!(parsear_ruta(".").unwrap(), vec![]);
        assert_eq!(
            parsear_ruta(".a[0].b[]").unwrap(),
            vec![Segmento::Clave("a".into()), Segmento::Indice(0), Segmento::Clave("b".into()), Segmento::Todos]
        );
        assert_eq!(parsear_ruta(".\"x y\"").unwrap(), vec![Segmento::Clave("x y".into())]);
        assert!(parsear_ruta(".a[").is_err());
        assert!(parsear_ruta("a").is_err());
    }

    #[test]
    fn extrae_con_fanout_e_indices_negativos() {
        let d = datos();
        let r = extraer(&d, &parsear_ruta(".usuarios[].nombre").unwrap());
        assert_eq!(r, vec![&json!("Ana"), &json!("Beto"), &json!("Caro")]);
        let r = extraer(&d, &parsear_ruta(".usuarios[-1].edad").unwrap());
        assert_eq!(r, vec![&json!(40)]);
        assert!(extraer(&d, &parsear_ruta(".no.existe").unwrap()).is_empty());
        assert_eq!(extraer(&d, &parsear_ruta(".meta.\"total en lista\"").unwrap()), vec![&json!(3)]);
    }

    #[test]
    fn filtra_por_condiciones() {
        let d = datos();
        let us = &d["usuarios"];
        let f = filtrar(us, &parsear_condicion(".edad >= 31").unwrap()).unwrap();
        assert_eq!(f.as_array().unwrap().len(), 2);
        let f = filtrar(us, &parsear_condicion(".tags ~ \"admin\"").unwrap()).unwrap();
        assert_eq!(f.as_array().unwrap().len(), 2);
        let f = filtrar(us, &parsear_condicion(".nombre ~ ar").unwrap()).unwrap(); // string sin comillas
        assert_eq!(f[0]["nombre"], "Caro");
        let f = filtrar(us, &parsear_condicion(".nombre != \"Ana\"").unwrap()).unwrap();
        assert_eq!(f.as_array().unwrap().len(), 2);
        assert!(filtrar(&json!(1), &parsear_condicion(".a == 1").unwrap()).is_err());
    }

    #[test]
    fn elige_campos() {
        let d = datos();
        let p = elegir(&d["usuarios"], &["nombre".into()]);
        assert_eq!(p[1], json!({"nombre":"Beto"}));
    }

    #[test]
    fn asigna_y_borra() {
        let mut d = datos();
        asignar(&mut d, &parsear_ruta(".usuarios[0].edad").unwrap(), json!(32)).unwrap();
        assert_eq!(d["usuarios"][0]["edad"], 32);
        asignar(&mut d, &parsear_ruta(".nuevo.anidado").unwrap(), json!(true)).unwrap();
        assert_eq!(d["nuevo"]["anidado"], true);
        assert!(asignar(&mut d, &parsear_ruta(".usuarios[9]").unwrap(), json!(1)).is_err());
        assert!(borrar(&mut d, &parsear_ruta(".usuarios[1]").unwrap()));
        assert_eq!(d["usuarios"].as_array().unwrap().len(), 2);
        assert!(borrar(&mut d, &parsear_ruta(".meta").unwrap()));
        assert!(!borrar(&mut d, &parsear_ruta(".meta").unwrap()));
    }
}
