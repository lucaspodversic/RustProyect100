# 08 · Watcher de logs con disparador

`logwatch` sigue un archivo como `tail -F` y ejecuta un comando cada vez que una línea nueva coincide con una regex.

## Qué demuestra
- Lectura incremental por offset, manejo de **líneas parciales**, **truncado** y **rotación** (cambio de inodo en Unix).
- Control de disparos con *cooldown* y máximo.
- Ejecución segura: la línea se pasa por la variable de entorno `WATCH_LINE`, **nunca interpolada** en el comando (evita inyección).
- Trait `Ejecutor` para testear sin lanzar procesos.

## Uso
```bash
cargo run -- /var/log/app.log 'ERROR|panic' 'echo "ALERTA: $WATCH_LINE" >> alertas.txt' --cooldown 30
cargo run -- app.log 'listo' 'echo terminado' --max 1 --desde-inicio
```

## Tests
`cargo test`: líneas nuevas/parciales, truncado, rotación, archivo que aparece después, cooldown/máximo, y un test Unix que verifica que contenido malicioso en el log no se ejecute.

## Limitaciones
- Polling (por defecto 500 ms) en vez de inotify/kqueue.
- En Windows no detecta rotación por inodo (sí por truncado).
- La acción se ejecuta de forma síncrona: un comando lento retrasa la lectura.
