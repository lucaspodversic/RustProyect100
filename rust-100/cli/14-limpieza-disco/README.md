# 14 · Limpieza de disco segura

`limpiar` encuentra espacio recuperable (artefactos de build, cachés, temporales viejos) y lo libera **solo si se pide**.

## Qué demuestra
- Defensa en profundidad para una operación destructiva:
  - categorías explícitas, con "archivo hermano" obligatorio (`target/` solo si hay `Cargo.toml`, `node_modules/` solo si hay `package.json`);
  - antigüedad mínima (mtime más reciente *dentro* del árbol);
  - `ruta_segura`: canonicalización, rechazo de symlinks, estrictamente dentro de raíces permitidas, nunca `/` ni home;
  - nunca entra en `.git`; temporales solo del usuario actual (Unix);
  - simulación por defecto, `--papelera` recuperable, re-validación antes de borrar.
- `walkdir` con `skip_current_dir` para no recorrer miles de `node_modules` anidados.

## Uso
```bash
cargo run -- -p ~/code                         # listar artefactos con +14 días
cargo run -- -p ~/code --caches -d 30
cargo run -- -p ~/code --papelera ~/.papelera  # mover (recuperable)
cargo run -- --temporales /tmp -d 7 --borrar
```

## Tests
`cargo test`: detección con/sin archivo hermano y exclusión de `.git`, cachés sin doble conteo, antigüedad, validación de rutas (traversal `..`, raíz, home, symlink), borrado y papelera, formato.

## Limitaciones
- La papelera usa `rename` (mismo disco); entre discos devuelve error en vez de copiar.
- "Dueño" en Unix se aproxima con el UID del `$HOME`.
- No integra la papelera nativa del SO (Trash/Recycle Bin).
