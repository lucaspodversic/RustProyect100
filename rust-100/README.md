# 100 proyectos de Rust

Desafío de 100 proyectos en Rust (edition 2021). Cada carpeta es un crate independiente con su propio `Cargo.toml`, `Cargo.lock`, tests y README en español. El código está comentado en español.

## Cómo usar

```bash
cd cli/01-renombrador-masivo
cargo test
```

Todos los tests corren offline: los servicios externos se simulan con mocks, SQLite en memoria, servidores locales y relojes inyectables.

## Resumen

| Categoría | Proyectos | Tests |
|---|---|---|
| CLI + Web/APIs | 01–30 | 163 |
| Sistemas + Redes | 31–65 | 75 |
| Blockchain + Juegos | 66–85 | 207 |
| Embebidos + Algoritmos | 86–100 | 175 |
| **Total** | **100** | **620** |

## Índice

### cli

- [01-renombrador-masivo](cli/01-renombrador-masivo/)
- [02-parser-logs](cli/02-parser-logs/)
- [03-buscador-local](cli/03-buscador-local/)
- [04-visor-disco-tui](cli/04-visor-disco-tui/)
- [05-procesador-json](cli/05-procesador-json/)
- [06-visor-procesos](cli/06-visor-procesos/)
- [07-benchmark-http](cli/07-benchmark-http/)
- [08-watcher-logs](cli/08-watcher-logs/)
- [09-todo-cli](cli/09-todo-cli/)
- [10-generador-regex](cli/10-generador-regex/)
- [11-auditor-red](cli/11-auditor-red/)
- [12-sincronizador-dirs](cli/12-sincronizador-dirs/)
- [13-cliente-imap-reglas](cli/13-cliente-imap-reglas/)
- [14-limpieza-disco](cli/14-limpieza-disco/)
- [15-grafo-deps-cargo](cli/15-grafo-deps-cargo/)

### web

- [16-api-rest-crud](web/16-api-rest-crud/)
- [17-acortador-urls](web/17-acortador-urls/)
- [18-auth-jwt](web/18-auth-jwt/)
- [19-backend-blog](web/19-backend-blog/)
- [20-chat-websocket](web/20-chat-websocket/)
- [21-api-todo](web/21-api-todo/)
- [22-subida-archivos](web/22-subida-archivos/)
- [23-api-clima](web/23-api-clima/)
- [24-acortador-analiticas](web/24-acortador-analiticas/)
- [25-webhooks](web/25-webhooks/)
- [26-graphql](web/26-graphql/)
- [27-fullstack-yew](web/27-fullstack-yew/)
- [28-portal-admin](web/28-portal-admin/)
- [29-api-reservas](web/29-api-reservas/)
- [30-rate-limiter](web/30-rate-limiter/)

### sistemas

- [31-mini-redis](sistemas/31-mini-redis/)
- [32-mini-kafka](sistemas/32-mini-kafka/)
- [33-mini-docker](sistemas/33-mini-docker/)
- [34-mini-git](sistemas/34-mini-git/)
- [35-mini-search](sistemas/35-mini-search/)
- [36-mini-compiler](sistemas/36-mini-compiler/)
- [37-mini-shell](sistemas/37-mini-shell/)
- [38-global-alloc](sistemas/38-global-alloc/)
- [39-fuse-basico](sistemas/39-fuse-basico/)
- [40-char-driver](sistemas/40-char-driver/)
- [41-http-server](sistemas/41-http-server/)
- [42-tcp-proxy](sistemas/42-tcp-proxy/)
- [43-util-linux](sistemas/43-util-linux/)
- [44-kernel-module](sistemas/44-kernel-module/)
- [45-quantum-sim](sistemas/45-quantum-sim/)
- [46-mini-db](sistemas/46-mini-db/)
- [47-dep-analyzer](sistemas/47-dep-analyzer/)
- [48-chip8](sistemas/48-chip8/)
- [49-generic-stack](sistemas/49-generic-stack/)
- [50-net-bench](sistemas/50-net-bench/)

### redes

- [51-tcp-chat](redes/51-tcp-chat/)
- [52-json-msg](redes/52-json-msg/)
- [53-dns-server](redes/53-dns-server/)
- [54-port-scanner](redes/54-port-scanner/)
- [55-file-transfer](redes/55-file-transfer/)
- [56-udp-game](redes/56-udp-game/)
- [57-matchmaking](redes/57-matchmaking/)
- [58-irc-client](redes/58-irc-client/)
- [59-socks5](redes/59-socks5/)
- [60-udp-echo-loss](redes/60-udp-echo-loss/)
- [61-mdns](redes/61-mdns/)
- [62-l4-balancer](redes/62-l4-balancer/)
- [63-tunnel](redes/63-tunnel/)
- [64-push-notify](redes/64-push-notify/)
- [65-rpc-bincode](redes/65-rpc-bincode/)

### blockchain

- [66-blockchain-minima](blockchain/66-blockchain-minima/)
- [67-nodo-p2p](blockchain/67-nodo-p2p/)
- [68-solana-staking](blockchain/68-solana-staking/)
- [69-explorador-tui](blockchain/69-explorador-tui/)
- [70-billetera](blockchain/70-billetera/)
- [71-arbol-merkle](blockchain/71-arbol-merkle/)
- [72-votacion](blockchain/72-votacion/)
- [73-indexador-eventos](blockchain/73-indexador-eventos/)
- [74-evm-simplificada](blockchain/74-evm-simplificada/)
- [75-escrow-atomic-swap](blockchain/75-escrow-atomic-swap/)

### juegos

- [76-shooter-2d](juegos/76-shooter-2d/)
- [77-snake](juegos/77-snake/)
- [78-plataformas](juegos/78-plataformas/)
- [79-visualizador-audio](juegos/79-visualizador-audio/)
- [80-editor-imagenes](juegos/80-editor-imagenes/)
- [81-cubo-rubik](juegos/81-cubo-rubik/)
- [82-particulas](juegos/82-particulas/)
- [83-reproductor-terminal](juegos/83-reproductor-terminal/)
- [84-pong-tcp](juegos/84-pong-tcp/)
- [85-topologia-pcie](juegos/85-topologia-pcie/)

### embebidos

- [86-estacion-meteorologica](embebidos/86-estacion-meteorologica/)
- [87-luces-ambientales](embebidos/87-luces-ambientales/)
- [88-reloj-despertador](embebidos/88-reloj-despertador/)
- [89-calidad-aire](embebidos/89-calidad-aire/)
- [90-riego-automatico](embebidos/90-riego-automatico/)
- [91-teclado-macros](embebidos/91-teclado-macros/)
- [92-driver-imu](embebidos/92-driver-imu/)
- [93-firmware-dron](embebidos/93-firmware-dron/)

### algoritmos

- [100-benchmark-concurrencia](algoritmos/100-benchmark-concurrencia/)
- [94-estructuras-de-datos](algoritmos/94-estructuras-de-datos/)
- [95-ordenamiento-paralelo](algoritmos/95-ordenamiento-paralelo/)
- [96-macro-derive](algoritmos/96-macro-derive/)
- [97-automatas](algoritmos/97-automatas/)
- [98-laberintos](algoritmos/98-laberintos/)
- [99-compresor](algoritmos/99-compresor/)

## Notas

- Proyectos educativos, no auditados para producción (en especial blockchain y seguridad).
- 27-fullstack-yew: el build WASM no se compiló en esta máquina; el README explica los pasos.
- Algunas dependencias están fijadas en `Cargo.lock` por compatibilidad con rustc 1.85.

Licencia: MIT.
