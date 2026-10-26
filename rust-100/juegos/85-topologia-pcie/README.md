# 85 · Visualizador de topología GPU/PCIe

Lee `/sys/bus/pci/devices` (**sólo lectura**, sin root) o un **sysfs simulado**, y muestra:

- **Árbol PCIe** reconstruido desde las rutas canónicas de sysfs (root complex → puertos/switches → dispositivos).
- Clasificación por código de clase: GPU / acelerador, puente, NVMe, red.
- **Enlace actual vs. máximo** (generación, ancho, GB/s útiles descontando la codificación 8b/10b o 128b/130b) y **alerta de enlace degradado** (p. ej. GPU x16 funcionando a x8).
- **Cuello de botella** de la cadena hasta el root complex.
- **Matriz de afinidad GPU×GPU**: `X`, `PIX`, `PXB`, `PHB`, `NODE`, `SYS`, según el ancestro común y el nodo NUMA.
- **TUI** con ratatui: árbol navegable (`j/k`), filtro sólo GPUs (`g`), detalle y matriz.

```bash
cargo test                    # 8 tests (sysfs simulado en un directorio temporal)
cargo run -- --demo           # servidor ficticio: 3 GPUs, 2 NUMA, una GPU degradada
cargo run -- --texto          # tu máquina real, salida en texto
cargo run -- --sysfs /ruta    # otra raíz (p. ej. un volcado de otra máquina)
```

## Limitaciones
Sólo Linux. No lee el espacio de configuración (capabilities, ACS, P2P real) ni NVLink/xGMI, que necesitarían herramientas del fabricante. La relación PIX/PXB es una aproximación basada en el árbol de sysfs. Los nombres de fabricante salen de una tabla mínima de IDs públicos.
