# 33 · Mini Docker (userspace)

Runtime de contenedores **educativo**. Simula namespaces, cgroups y OverlayFS sin privilegios. El camino real de Linux se documenta tras el feature `linux`.

## Qué demuestra
- Namespaces (pid/mount/net/uts/user/ipc) como conjuntos aislados; compartir netns.
- Cgroups: `memory.max` (OOM) y `cpu.max` (throttling) con contadores.
- OverlayFS: capas lower + upper, copy-on-write y *whiteouts*.
- Ciclo de vida create → start → stop → remove.

## Cómo correr / testear
```bash
cargo test
# El feature `linux` solo documenta el path real; no se ejecuta en CI.
```

## Limitaciones
No aísla de verdad (es un modelo). Sin red/iptables, sin seccomp, sin imágenes OCI. En un host real haría falta `CAP_SYS_ADMIN` + cgroup v2.
