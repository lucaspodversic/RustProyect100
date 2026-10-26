# 82 · Simulación de partículas

Motor de partículas 2D, sin dependencias ni GPU.

- **Euler semi-implícito** (simpléctico): órbitas estables y buena conservación de energía.
- **Fuerzas**: gravedad, viento (depende de la masa), atractor/repulsor suavizado y arrastre lineal.
- **Emisores** con tasa fraccional, cono de dirección, rango de rapidez y vida, más un tope de partículas.
- **Paredes** con coeficiente de restitución.
- **Colisiones elásticas** entre esferas, con **grilla espacial**, verificada contra fuerza bruta.
- **Render ASCII** por densidad (` .:*#@`) y RNG determinista.

```bash
cargo test           # 9 tests (caída libre, órbita, momento, energía, grilla…)
cargo run -- 5       # fuente de partículas durante 5 s
```

## Limitaciones
Colisiones resueltas en un solo pase por paso, así que con mucha densidad puede quedar algo de solapamiento. Sin detección continua (CCD), lo que provoca *tunneling* a velocidades muy altas con `dt` grande.
