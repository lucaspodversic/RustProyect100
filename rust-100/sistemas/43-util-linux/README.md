# 43 · util-linux (mini)

Reimplementación de `tail`, `mountpoint` y `wc`.

## Limitaciones
`mountpoint` usa comparación de `st_dev` (no parsea `/proc/self/mountinfo`). `tail -f` es un único "once".
