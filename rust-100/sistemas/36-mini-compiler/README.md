# 36 · Mini Compilador/Intérprete

Lenguaje "Calc": `let`, aritmética, paréntesis. Pipeline lexer → parser Pratt → AST → eval.

## Tests
`cargo test` — precedencia, variables, errores (división por cero, var indefinida, lex).

## Limitaciones
Sin funciones, sin tipos, sin bytecode/JIT: evalúa el AST directamente.
