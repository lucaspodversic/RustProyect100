# 37 · Shell Unix simplificado

Parser de pipes/redirecciones + ejecución con `Command`/`Stdio`. Builtins: `cd`, `pwd`, `exit`.

## Tests
`cargo test` — parseo y pipeline real (`printf | sort > file`).

## Limitaciones
Sin comillas/escaping, sin `$VAR`, sin job control, sin globs.
