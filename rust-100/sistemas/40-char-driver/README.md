# 40 · Driver de caracteres (mock)

Char device con buffer circular, bloqueo/`EAGAIN`, ioctl y registro tipo `/dev`. Feature `linux` documenta el esqueleto del módulo real.

## Limitaciones
Userspace puro: no habla con el kernel ni crea nodos en `/dev`.
