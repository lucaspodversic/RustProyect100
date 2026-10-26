# 04 · Visor de uso de disco con TUI (ratatui)

`dutui` escanea un directorio, calcula el tamaño acumulado de cada subdirectorio y lo muestra en una interfaz de terminal navegable, ordenado de mayor a menor, con barras proporcionales y un gauge de la selección.

## Qué demuestra
- Árbol recursivo con tamaños acumulados (sin seguir symlinks → sin ciclos).
- Patrón **modelo / estado / vista**: `App` es testeable sin terminal; la vista se renderiza en `TestBackend`.
- `ratatui` (List con estado, Gauge, Layout) + `crossterm` (raw mode, pantalla alternativa, restauración segura).

## Uso
```bash
cargo run -- ~/proyectos         # TUI
cargo run -- . --texto 10        # top-10 en texto, sin TUI
```
Teclas: `↑/k` `↓/j` mover · `Enter/l/→` entrar · `Backspace/h/←` subir · `q/Esc` salir.

## Tests (sin display)
`cargo test`: escaneo, formato humano, navegación y **render headless** con `ratatui::backend::TestBackend` (se verifica el contenido del buffer). No se necesita una terminal real.

## Limitaciones
- Escaneo secuencial y completo antes de mostrar (en discos enormes tarda).
- Tamaño aparente (`len`), no bloques ocupados en disco; no deduplica hardlinks.
