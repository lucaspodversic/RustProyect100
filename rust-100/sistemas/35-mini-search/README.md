# 35 · Mini Search Engine

Índice invertido con ranking **TF-IDF** y similitud del coseno.

## Qué demuestra
Tokenización Unicode, postings término→(doc,tf), IDF suavizado, vectores TF-IDF y ranking por coseno.

## Tests
`cargo test` — ranking, Unicode, índice vacío.

## Limitaciones
Sin stemming/lemmatization, sin posiciones (no frases), sin persistencia.
