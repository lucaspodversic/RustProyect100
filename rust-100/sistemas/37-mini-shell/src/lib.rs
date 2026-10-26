//! # Shell Unix simplificado
//!
//! Parsea líneas con **pipes** (`|`) y **redirecciones** (`>`, `>>`, `<`),
//! y las ejecuta con `std::process::Command` + `Stdio`.
//!
//! Builtins: `cd`, `pwd`, `exit`. El resto se busca en `$PATH`.

use std::env;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Una etapa del pipeline: comando + redirecciones propias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    pub argv: Vec<String>,
    pub stdin_file: Option<PathBuf>,
    pub stdout_file: Option<(PathBuf, bool)>, // bool = append
}

/// Parsea una línea. No interpreta comillas (versión educativa).
pub fn parse(line: &str) -> Result<Vec<Stage>, String> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(vec![]);
    }
    let mut stages = Vec::new();
    for part in line.split('|') {
        let mut argv = Vec::new();
        let mut stdin_file = None;
        let mut stdout_file = None;
        let mut words = part.split_whitespace().peekable();
        while let Some(w) = words.next() {
            match w {
                "<" => {
                    let f = words.next().ok_or("falta archivo tras <")?;
                    stdin_file = Some(f.into());
                }
                ">" => {
                    let f = words.next().ok_or("falta archivo tras >")?;
                    stdout_file = Some((f.into(), false));
                }
                ">>" => {
                    let f = words.next().ok_or("falta archivo tras >>")?;
                    stdout_file = Some((f.into(), true));
                }
                _ => argv.push(w.into()),
            }
        }
        if argv.is_empty() {
            return Err("comando vacío en el pipeline".into());
        }
        stages.push(Stage { argv, stdin_file, stdout_file });
    }
    Ok(stages)
}

/// Ejecuta un pipeline. Devuelve el código de salida de la última etapa.
pub fn run_pipeline(stages: &[Stage]) -> io::Result<i32> {
    if stages.is_empty() {
        return Ok(0);
    }
    // Builtins sólo en pipeline de una etapa sin redirecciones raras.
    if stages.len() == 1 && stages[0].stdin_file.is_none() && stages[0].stdout_file.is_none() {
        match stages[0].argv[0].as_str() {
            "cd" => {
                let dir = stages[0].argv.get(1).map(String::as_str).unwrap_or(".");
                env::set_current_dir(dir)?;
                return Ok(0);
            }
            "pwd" => {
                println!("{}", env::current_dir()?.display());
                return Ok(0);
            }
            "exit" => std::process::exit(stages[0].argv.get(1).and_then(|s| s.parse().ok()).unwrap_or(0)),
            _ => {}
        }
    }

    let mut children = Vec::new();
    let mut prev_stdout: Option<Stdio> = None;
    for (i, st) in stages.iter().enumerate() {
        let mut cmd = Command::new(&st.argv[0]);
        cmd.args(&st.argv[1..]);
        // stdin
        if let Some(f) = &st.stdin_file {
            cmd.stdin(File::open(f)?);
        } else if let Some(p) = prev_stdout.take() {
            cmd.stdin(p);
        }
        // stdout
        let is_last = i + 1 == stages.len();
        if let Some((f, append)) = &st.stdout_file {
            let file = if *append {
                OpenOptions::new().create(true).append(true).open(f)?
            } else {
                File::create(f)?
            };
            cmd.stdout(file);
            prev_stdout = None;
        } else if !is_last {
            cmd.stdout(Stdio::piped());
        }
        let mut child = cmd.spawn()?;
        if !is_last {
            // El pipe de salida de este proceso alimenta al siguiente.
            prev_stdout = child.stdout.take().map(Stdio::from);
        }
        children.push(child);
    }
    let mut code = 0;
    for mut c in children {
        code = c.wait()?.code().unwrap_or(1);
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_pipes_y_redir() {
        let p = parse("cat a.txt | sort > out.txt").unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].argv, vec!["cat", "a.txt"]);
        assert_eq!(p[1].stdout_file, Some(("out.txt".into(), false)));
        let p = parse("echo hola >> log").unwrap();
        assert_eq!(p[0].stdout_file, Some(("log".into(), true)));
    }

    #[test]
    fn pipeline_real() {
        let dir = std::env::temp_dir().join(format!("mshell-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let inp = dir.join("in.txt");
        let out = dir.join("out.txt");
        fs::write(&inp, b"b
a
c
").unwrap();
        // cat archivo | sort > out  (sin comillas: el parser no las soporta)
        let stages = parse(&format!("cat {} | sort > {}", inp.display(), out.display())).unwrap();
        assert_eq!(run_pipeline(&stages).unwrap(), 0);
        assert_eq!(fs::read_to_string(&out).unwrap(), "a
b
c
");
        let stages = parse(&format!("cd {}", dir.display())).unwrap();
        assert_eq!(run_pipeline(&stages).unwrap(), 0);
        assert_eq!(env::current_dir().unwrap(), dir);
        fs::remove_dir_all(&dir).unwrap();
    }
}
