# 10 · Generador de regex desde ejemplos (tipo grex)

`regexgen` recibe ejemplos y produce una regex anclada que los acepta. Sin opciones, acepta **exactamente** esos ejemplos; con `-d/-p/-e/-r` generaliza.

## Algoritmo
1. **Tokenización**: cada carácter → literal o clase (`\d`, `\w`, `\s`); repeticiones → `{n}`.
2. **Trie** de tokens: factoriza prefijos comunes.
3. **Render recursivo**: alternancias `(?:a|b)`, clases `[abc]` / rangos `[a-e]`, sufijos opcionales `(?:...)?`.
4. **Verificación**: la regex se compila y se prueba contra todos los ejemplos.

| Ejemplos | Resultado |
|---|---|
| `abc abd` | `^ab[cd]$` |
| `hola holanda` | `^hola(?:nda)?$` |
| `2024-01-05 1999-12-31` con `-d -r` | `^\d{4}\-\d{2}\-\d{2}$` |

## Uso
```bash
cargo run -- perro gato
cargo run -- 2024-01-05 1999-12-31 -d -r
cargo run -- -f ejemplos.txt -i
```

## Tests
`cargo test`: escapado, prefijos, clases, rangos, opcionales, exactitud (acepta ejemplos y rechaza otros), generalizaciones.

## Limitaciones
- No factoriza **sufijos** comunes (grex sí), así que algunas regex son más largas de lo óptimo.
- No detecta repeticiones de subcadenas (`abab` → `(?:ab){2}`).
