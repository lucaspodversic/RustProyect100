# 11 · Auditor de red local (tipo mynet)

`mired` lista interfaces, IPs (IPv4/IPv6) con su clasificación, subred (CIDR, red, broadcast, hosts), gateway por defecto y DNS. Opcionalmente chequea puertos TCP del **propio gateway**.

## Qué demuestra
- `if-addrs` para enumerar interfaces en Linux/macOS/Windows.
- Aritmética de subredes con `u32` (máscara → prefijo, red, broadcast).
- Clasificación de rangos (RFC1918, CGNAT 100.64/10, link-local, ULA fc00::/7).
- **Parsers puros y testeables** para la tabla de rutas de cada SO (`/proc/net/route`, `netstat -rn`, `route print`).
- `cfg(target_os)` para código específico de plataforma.

## Uso
```bash
cargo run
cargo run -- --sin-loopback --json
cargo run -- --puertos 22,53,80,443      # solo en tu propia red
```

## Tests
`cargo test`: clasificación, subredes, parsers de gateway con salidas reales de ejemplo de los 3 SO, `resolv.conf`, interfaces reales (debe existir loopback) y chequeo de puertos contra un listener local.

## Limitaciones
- DNS solo vía `/etc/resolv.conf` (en Windows no se detecta).
- Gateway solo IPv4.
- El chequeo de puertos es un `connect` TCP simple; usalo únicamente en redes propias o autorizadas.
