//! # Base de datos relacional mínima
//!
//! Tablas en memoria, parser SQL reducido y JOIN anidado por bucles.
//! Soporte: `CREATE TABLE`, `INSERT INTO`, `SELECT ... FROM ... [JOIN ... ON] [WHERE]`.

use std::collections::HashMap;

pub type Row = Vec<String>;

#[derive(Debug, Clone)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
}

#[derive(Debug, Default)]
pub struct Database {
    pub tables: HashMap<String, Table>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Sql {
    Create { name: String, cols: Vec<String> },
    Insert { table: String, values: Vec<String> },
    Select {
        cols: Vec<String>, // "*" o lista
        from: String,
        join: Option<(String, String, String)>, // (tabla, left_col, right_col)
        where_eq: Option<(String, String)>,     // col = valor
    },
}

/// Tokenizer minimalista: respeta strings entre comillas simples.
pub fn tokenize(sql: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    for ch in sql.chars() {
        if in_str {
            if ch == '\'' {
                out.push(std::mem::take(&mut cur));
                in_str = false;
            } else {
                cur.push(ch);
            }
        } else {
            match ch {
                '\'' => in_str = true,
                ' ' | '\n' | '\t' | ',' | ';' => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                '(' | ')' | '=' | '*' => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                    out.push(ch.to_string());
                }
                _ => cur.push(ch),
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn parse(sql: &str) -> Result<Sql, String> {
    let t = tokenize(sql);
    let up = |s: &str| s.to_ascii_uppercase();
    if t.is_empty() {
        return Err("vacío".into());
    }
    match up(&t[0]).as_str() {
        "CREATE" => {
            // CREATE TABLE name (c1 c2 ...)
            if t.len() < 5 || up(&t[1]) != "TABLE" {
                return Err("CREATE TABLE ...".into());
            }
            let name = t[2].clone();
            let mut cols = Vec::new();
            let mut i = 4; // salta '('
            while i < t.len() && t[i] != ")" {
                cols.push(t[i].clone());
                i += 1;
            }
            Ok(Sql::Create { name, cols })
        }
        "INSERT" => {
            // INSERT INTO name VALUES (v1 v2)
            if up(&t[1]) != "INTO" || up(&t[3]) != "VALUES" {
                return Err("INSERT INTO t VALUES (...)".into());
            }
            let table = t[2].clone();
            let mut values = Vec::new();
            let mut i = 5;
            while i < t.len() && t[i] != ")" {
                values.push(t[i].clone());
                i += 1;
            }
            Ok(Sql::Insert { table, values })
        }
        "SELECT" => {
            let mut i = 1;
            let mut cols = Vec::new();
            if t[i] == "*" {
                cols.push("*".into());
                i += 1;
            } else {
                while up(&t[i]) != "FROM" {
                    cols.push(t[i].clone());
                    i += 1;
                }
            }
            if up(&t[i]) != "FROM" {
                return Err("falta FROM".into());
            }
            i += 1;
            let from = t[i].clone();
            i += 1;
            let mut join = None;
            if i < t.len() && up(&t[i]) == "JOIN" {
                let jt = t[i + 1].clone();
                // ON a.col = b.col  → tomamos los nombres de columna sin calificar
                let left = t[i + 3].rsplit('.').next().unwrap().to_string();
                let right = t[i + 5].rsplit('.').next().unwrap().to_string();
                join = Some((jt, left, right));
                i += 6;
            }
            let mut where_eq = None;
            if i < t.len() && up(&t[i]) == "WHERE" {
                let col = t[i + 1].rsplit('.').next().unwrap().to_string();
                let val = t[i + 3].clone();
                where_eq = Some((col, val));
            }
            Ok(Sql::Select { cols, from, join, where_eq })
        }
        _ => Err("solo CREATE/INSERT/SELECT".into()),
    }
}

impl Database {
    pub fn new() -> Self { Self::default() }

    pub fn exec(&mut self, sql: &str) -> Result<Vec<Row>, String> {
        match parse(sql)? {
            Sql::Create { name, cols } => {
                self.tables.insert(name, Table { columns: cols, rows: Vec::new() });
                Ok(vec![])
            }
            Sql::Insert { table, values } => {
                let t = self.tables.get_mut(&table).ok_or("tabla inexistente")?;
                if values.len() != t.columns.len() {
                    return Err("arity".into());
                }
                t.rows.push(values);
                Ok(vec![])
            }
            Sql::Select { cols, from, join, where_eq } => {
                let left = self.tables.get(&from).ok_or("tabla inexistente")?.clone();
                let mut result_cols = left.columns.clone();
                let mut result_rows: Vec<Row> = left.rows.clone();
                if let Some((jt, lc, rc)) = join {
                    let right = self.tables.get(&jt).ok_or("tabla join inexistente")?.clone();
                    let li = left.columns.iter().position(|c| c == &lc).ok_or("col left")?;
                    let ri = right.columns.iter().position(|c| c == &rc).ok_or("col right")?;
                    let mut joined = Vec::new();
                    for lr in &left.rows {
                        for rr in &right.rows {
                            if lr[li] == rr[ri] {
                                let mut row = lr.clone();
                                row.extend(rr.clone());
                                joined.push(row);
                            }
                        }
                    }
                    result_cols.extend(right.columns.clone());
                    result_rows = joined;
                }
                if let Some((col, val)) = where_eq {
                    let i = result_cols.iter().position(|c| c == &col).ok_or("col where")?;
                    result_rows.retain(|r| r[i] == val);
                }
                if cols.len() == 1 && cols[0] == "*" {
                    Ok(result_rows)
                } else {
                    let idxs: Result<Vec<_>, _> = cols
                        .iter()
                        .map(|c| result_cols.iter().position(|x| x == c).ok_or_else(|| format!("col {c}")))
                        .collect();
                    let idxs = idxs?;
                    Ok(result_rows.into_iter().map(|r| idxs.iter().map(|&i| r[i].clone()).collect()).collect())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn create_insert_select_join() {
        let mut db = Database::new();
        db.exec("CREATE TABLE users (id name)").unwrap();
        db.exec("CREATE TABLE orders (id user_id total)").unwrap();
        db.exec("INSERT INTO users VALUES (1 'Ada')").unwrap();
        db.exec("INSERT INTO users VALUES (2 'Grace')").unwrap();
        db.exec("INSERT INTO orders VALUES (10 1 '100')").unwrap();
        db.exec("INSERT INTO orders VALUES (11 1 '50')").unwrap();
        db.exec("INSERT INTO orders VALUES (12 2 '70')").unwrap();
        let rows = db
            .exec("SELECT name total FROM users JOIN orders ON users.id = orders.user_id WHERE name = 'Ada'")
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], "Ada");
        let all = db.exec("SELECT * FROM users").unwrap();
        assert_eq!(all.len(), 2);
    }
}
