//! # Mini Compilador / Intérprete
//!
//! Lenguaje de juguete "Calc":
//! ```text
//! let x = 1 + 2 * 3;
//! let y = x - 4;
//! y
//! ```
//! Pipeline: **lexer → parser (Pratt) → AST → evaluador**.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Num(f64),
    Ident(String),
    Let,
    Eq,
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Semi,
    Eof,
}

pub fn lex(src: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            c if c.is_whitespace() => i += 1,
            '+' => { out.push(Token::Plus); i += 1; }
            '-' => { out.push(Token::Minus); i += 1; }
            '*' => { out.push(Token::Star); i += 1; }
            '/' => { out.push(Token::Slash); i += 1; }
            '=' => { out.push(Token::Eq); i += 1; }
            '(' => { out.push(Token::LParen); i += 1; }
            ')' => { out.push(Token::RParen); i += 1; }
            ';' => { out.push(Token::Semi); i += 1; }
            c if c.is_ascii_digit() || c == '.' => {
                let s = i;
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let n: f64 = chars[s..i].iter().collect::<String>().parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
                out.push(Token::Num(n));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let s = i;
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let w: String = chars[s..i].iter().collect();
                out.push(if w == "let" { Token::Let } else { Token::Ident(w) });
            }
            c => return Err(format!("carácter inesperado: {c}")),
        }
    }
    out.push(Token::Eof);
    Ok(out)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Var(String),
    Binary(Box<Expr>, BinOp, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp { Add, Sub, Mul, Div }

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let(String, Expr),
    Expr(Expr),
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }
    fn cur(&self) -> &Token { &self.tokens[self.pos] }
    fn bump(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() { self.pos += 1; }
        t
    }
    fn expect(&mut self, want: &Token) -> Result<(), String> {
        if self.cur() == want { self.bump(); Ok(()) } else {
            Err(format!("se esperaba {want:?}, hay {:?}", self.cur()))
        }
    }

    /// Precedencia Pratt: mayor = más apretado.
    fn prec(t: &Token) -> u8 {
        match t {
            Token::Plus | Token::Minus => 10,
            Token::Star | Token::Slash => 20,
            _ => 0,
        }
    }

    pub fn parse_program(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        while !matches!(self.cur(), Token::Eof) {
            stmts.push(self.parse_stmt()?);
            if matches!(self.cur(), Token::Semi) { self.bump(); }
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        if matches!(self.cur(), Token::Let) {
            self.bump();
            let Token::Ident(name) = self.bump() else { return Err("ident".into()) };
            self.expect(&Token::Eq)?;
            Ok(Stmt::Let(name, self.parse_expr(0)?))
        } else {
            Ok(Stmt::Expr(self.parse_expr(0)?))
        }
    }

    fn parse_expr(&mut self, min_prec: u8) -> Result<Expr, String> {
        let mut left = self.parse_primary()?;
        while Self::prec(self.cur()) > min_prec {
            let op = match self.bump() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                t => return Err(format!("op {t:?}")),
            };
            let right = self.parse_expr(Self::prec(&match op {
                BinOp::Add | BinOp::Sub => Token::Plus,
                BinOp::Mul | BinOp::Div => Token::Star,
            }))?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.bump() {
            Token::Num(n) => Ok(Expr::Num(n)),
            Token::Ident(s) => Ok(Expr::Var(s)),
            Token::LParen => {
                let e = self.parse_expr(0)?;
                self.expect(&Token::RParen)?;
                Ok(e)
            }
            Token::Minus => Ok(Expr::Binary(Box::new(Expr::Num(0.0)), BinOp::Sub, Box::new(self.parse_primary()?))),
            t => Err(format!("primario inesperado: {t:?}")),
        }
    }
}

pub fn eval_expr(e: &Expr, env: &HashMap<String, f64>) -> Result<f64, String> {
    match e {
        Expr::Num(n) => Ok(*n),
        Expr::Var(v) => env.get(v).copied().ok_or_else(|| format!("variable no definida: {v}")),
        Expr::Binary(a, op, b) => {
            let (a, b) = (eval_expr(a, env)?, eval_expr(b, env)?);
            Ok(match op {
                BinOp::Add => a + b,
                BinOp::Sub => a - b,
                BinOp::Mul => a * b,
                BinOp::Div => {
                    if b == 0.0 { return Err("división por cero".into()); }
                    a / b
                }
            })
        }
    }
}

pub fn run(src: &str) -> Result<f64, String> {
    let tokens = lex(src)?;
    let stmts = Parser::new(tokens).parse_program()?;
    let mut env = HashMap::new();
    let mut last = 0.0;
    for s in stmts {
        match s {
            Stmt::Let(n, e) => { let v = eval_expr(&e, &env)?; env.insert(n, v); last = v; }
            Stmt::Expr(e) => last = eval_expr(&e, &env)?,
        }
    }
    Ok(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precedencia() {
        assert_eq!(run("1 + 2 * 3").unwrap(), 7.0);
        assert_eq!(run("(1 + 2) * 3").unwrap(), 9.0);
    }
    #[test]
    fn variables() {
        assert_eq!(run("let x = 10; let y = x - 4; y").unwrap(), 6.0);
    }
    #[test]
    fn errores() {
        assert!(run("1 / 0").unwrap_err().contains("cero"));
        assert!(run("z").unwrap_err().contains("no definida"));
        assert!(lex("@").is_err());
    }
}
