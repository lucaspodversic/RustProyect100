# 06 · Visor de procesos interactivo

`procs` es un `top` simplificado: tabla de procesos con CPU %, memoria (MiB y %), ordenable por CPU / memoria / PID / nombre y con filtro en vivo.

## Qué demuestra
- Uso de `sysinfo` (multiplataforma: Linux, macOS, Windows).
- **Inyección de dependencias con traits** (`FuenteProcesos`): la lógica se testea con una fuente falsa determinista.
- Bucle de eventos con `crossterm::event::poll` + refresco periódico.
- Tabla con estado de `ratatui`.

## Uso
```bash
cargo run --release                # TUI
cargo run -- --intervalo 500       # refresco cada 500 ms
cargo run -- --top 10              # modo texto
```
Teclas: `j/k` mover · `s` rotar orden · `c` CPU · `m` memoria · `/` filtrar (Enter/Esc termina) · `r` refrescar · `q` salir.

## Tests
`cargo test`: orden, filtro, teclas, render headless (`TestBackend`) y un test contra el sistema real que verifica que aparezca el PID del propio test.

## Limitaciones
- El % de CPU necesita dos lecturas: el primer cuadro muestra 0.
- No permite matar procesos (decisión deliberada de seguridad).
