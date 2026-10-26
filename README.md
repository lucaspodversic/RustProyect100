
<div align="center">

# **100 proyectos de Rust**

</div>




### 🎯 Categorías para un Portafolio de Impacto



1.  **Herramientas CLI que resuelven un problema real:** Un renombrador de archivos, un parser de logs o un buscador local que tú mismo uses demuestra que sabes construir software útil, no solo de tutorial.
2.  **Servicios HTTP con base de datos real:** Un API construido con `axum` + `sqlx` + PostgreSQL, con al menos un endpoint funcional y persistencia, demuestra que entiendes el desarrollo backend moderno.
3.  **Contribuciones a proyectos open source existentes:** Arreglar un bug o añadir una feature a un crate conocido es una señal de madurez técnica muy valorada.

### 📋 100 Proyectos Rust Organizados por Área




#### 🖥️ CLI Tools y Utilidades (15 proyectos)
*Enfoque: Ergonomía, parsing de argumentos, I/O de archivos, concurrencia simple.*

1.  **Renombrador de archivos masivo** con soporte para regex y previsualización de cambios.
2.  **Parser de logs** que filtre por nivel, fecha o patrón y muestre estadísticas (ej. errores por hora).
3.  **Buscador de archivos local** (tipo `ripgrep` simplificado) con búsqueda por nombre y contenido.
4.  **Visor de uso de disco** con TUI (usando `ratatui`) que muestre los directorios más pesados.
5.  **Procesador de JSON** para extraer, filtrar y transformar campos desde la línea de comandos.
6.  **Visor de procesos** interactivo con ordenación por CPU/memoria.
7.  **Herramienta de benchmarking HTTP** (tipo `wrk` o `hey` simplificado) con concurrencia.
8.  **Watcher de logs** que ejecute un comando cuando aparezca un patrón específico.
9.  **Gestor de tareas (To-Do)** en CLI con persistencia en un archivo local (SQLite o JSON).
10. **Generador de expresiones regulares** a partir de casos de prueba (inspirado en `grex`).
11. **Auditor de red local** que liste IPs, interfaces y gateway (tipo `mynet`).
12. **Sincronizador de archivos** básico entre dos directorios (tipo `rsync` simplificado).
13. **Cliente de correo** para triaje por IMAP con reglas de filtrado.
14. **Herramienta de limpieza de disco** que detecte cachés y archivos temporales de forma segura.
15. **Visualizador de dependencias** de un proyecto (Cargo) como grafo interactivo.

#### 

🌐 Web, APIs y Backend (15 proyectos)
*Enfoque: Async/await, frameworks (Axum, Actix), bases de datos, autenticación.*

16. **API REST con Axum + SQLx + PostgreSQL** para gestionar una entidad (ej. libros, películas) con CRUD completo.
17. **Servicio de acortamiento de URLs** con redirección y estadísticas básicas de clics.
18. **API de autenticación** con JWT, registro, login y protección de rutas.
19. **Backend para un blog** con posts, comentarios y etiquetas, usando ORM o SQLx.
20. **WebSocket server** para un chat en tiempo real con salas y historial simple.
21. **API de tareas (To-Do)** con listas, ítems y marcado de completado.
22. **Servicio de subida y descarga de archivos** con límites de tamaño y tipos MIME.
23. **API de clima** que consuma una API externa y cachee respuestas.
24. **Backend para un acortador de enlaces con analíticas** (clics por día, referrer).
25. **Servicio de webhooks** que reciba eventos y los reenvíe a otros endpoints.
26. **API GraphQL** sobre un esquema simple usando `async-graphql`.
27. **Aplicación full-stack con Leptos o Yew** (WASM) conectada a un backend Rust.
28. **Portal de administración** con autenticación y panel para gestionar contenido.
29. **API de reservas** (ej. canchas, salas) con disponibilidad y conflictos de horario.
30. **Rate limiter** como middleware para un servidor Axum/Actix.



#### ⚙️ Sistemas, Herramientas y Bajo Nivel (20 proyectos)
*Enfoque: Memoria, concurrencia, protocolos, sistemas operativos, compiladores.*

31. **Mini Redis**: Almacén clave-valor compatible con el protocolo RESP, con persistencia y cliente CLI.
32. **Mini Kafka**: Cola de mensajes distribuida con particiones y consumidores.
33. **Mini Docker**: Runtime de contenedores que use `namespaces` y `cgroups` de Linux.
34. **Mini Git**: Sistema de control de versiones con almacenamiento direccionado por contenido.
35. **Mini Search Engine**: Indexación de texto completo con ranking TF-IDF.
36. **Mini Compilador/Intérprete** para un lenguaje de juguete (lexer, parser, AST, evaluación).
37. **Shell de Unix simplificado** con creación de procesos, redirección y tuberías.
38. **Asignador de memoria global** personalizado (`GlobalAlloc`).
39. **Sistema de archivos en espacio de usuario** (FUSE) básico.
40. **Driver de dispositivo de caracteres** simple para Linux.
41. **Servidor HTTP/1.1 desde cero** sobre TCP, sin frameworks.
42. **Proxy inverso TCP asíncrono** para balanceo de carga.
43. **Implementación de `util-linux`**: Rehacer herramientas como `mountpoint` o `tail`.
44. **Módulo de kernel** de juguete (toy) en Rust.
45. **Simulador cuántico básico** (operaciones con qubits y compuertas).
46. **Base de datos relacional mínima** con parser SQL y joins simples.
47. **Analizador de dependencias de código** que detecte ciclos y visualice el grafo.
48. **Emulador de una CPU de 8 bits** (ej. CHIP-8) con intérprete de instrucciones.
49. **Sistema de migas de pan (stack) genérico** con traits y genéricos.
50. **Herramienta de benchmarking de red** que mida latencia y throughput real contra endpoints.

#### 🔌 Redes y Protocolos (15 proyectos)
*Enfoque: Sockets, TCP/UDP, protocolos de aplicación, async networking.*

51. **Servidor de chat TCP** multihilo con nombres de usuario y salas.
52. **Cliente y servidor de un protocolo de mensajería propio** (ej. basado en JSON sobre TCP).
53. **Servidor DNS simple** que resuelva dominios desde un archivo de zona.
54. **Escáner de puertos** concurrente con timeouts configurables.
55. **Servidor de transferencia de archivos** sobre TCP con progreso.
56. **Implementación de un protocolo de juego** (ej. sincronización de estado) sobre UDP.
57. **Servidor de matchmaking** para juegos online usando WebSockets.
58. **Cliente de IRC** básico para conectarse a un servidor y unirse a canales.
59. **Proxy SOCKS5** simple.
60. **Servidor de eco UDP** con manejo de paquetes perdidos.
61. **Implementación de un protocolo de descubrimiento** (tipo mDNS simplificado).
62. **Balanceador de carga L4** que distribuya conexiones TCP entre backends.
63. **Herramienta de tunneling** (tipo `ngrok` básico) que exponga un puerto local.
64. **Servidor de notificaciones push** usando WebSockets y colas.
65. **Cliente/servidor de RPC** con serialización binaria (ej. bincode o protobuf).

#### ⛓️ Blockchain y Cripto (10 proyectos)
*Enfoque: Criptografía, estructuras de datos inmutables, consenso, smart contracts.*

66. **Implementación de una blockchain mínima** con bloques, pruebas de trabajo (PoW) y validación de cadena.
67. **Nodo de una red blockchain** que sincronice bloques con otros pares.
68. **Smart contract en Solana** usando Anchor (ej. escrow o staking).
69. **Explorador de bloques** en TUI para una blockchain (ej. SOON o similar).
70. **Billetera de criptomonedas** que genere direcciones y firme transacciones.
71. **Implementación de un árbol de Merkle** para verificación de transacciones.
72. **Contrato inteligente de votación** descentralizada.
73. **Indexador de eventos** on-chain para una DApp.
74. **Máquina virtual de Ethereum (EVM) simplificada** que ejecute opcodes básicos.
75. **Sistema de escrow con atomic swap** entre dos partes.



#### 🎮 Gráficos, Juegos y Multimedia (10 proyectos)
*Enfoque: Motores (Bevy, Macroquad), renderizado, física, audio.*

76. **Juego de naves (shooter)** 2D con Bevy o Macroquad, con enemigos y puntuación.
77. **Clon de Snake** con puntuación máxima persistente.
78. **Juego de plataformas** simple con físicas básicas y colisiones.
79. **Visualizador de audio** que reaccione a la música en tiempo real.
80. **Editor de imágenes** básico (filtros, recorte, redimensionado).
81. **Juego de cubo de Rubik** interactivo con Bevy.
82. **Simulación de partículas** con renderizado en tiempo real.
83. **Reproductor de música** en terminal con controles de reproducción.
84. **Juego multijugador local** (ej. Pong) con red TCP.
85. **Visualizador de topología de GPU/PCIe** (inspirado en herramientas de sistemas).



#### 🤖 Embebidos, IoT y Tiempo Real (8 proyectos)
*Enfoque: `no_std`, microcontroladores (ESP32, STM32, RP2040), sensores.*

86. **Estación meteorológica IoT** con sensor de temperatura/humedad y envío de datos por Wi-Fi (ESP32).
87. **Controlador de luces ambientales** que ajuste el color según la hora o sensores.
88. **Reloj despertador** con pantalla OLED y botones en Raspberry Pi Pico.
89. **Monitor de calidad del aire** con sensor y display.
90. **Sistema de riego automático** basado en humedad del suelo.
91. **Teclado macro** personalizado con RP2040.
92. **Driver para un sensor específico** (ej. IMU, pantalla LCD) usando `embedded-hal`.
93. **Firmware para un dron** básico (control de motores y lectura de IMU).



#### 🧠 Lenguaje, Algoritmos y Rendimiento (7 proyectos)
*Enfoque: Traits, genéricos, macros, concurrencia, optimización.*

94. **Implementación de estructuras de datos** (lista enlazada, árbol binario, hash map) desde cero.
95. **Algoritmo de ordenamiento paralelo** usando `rayon` y comparación de rendimiento.
96. **Macro procedural** que genere código boilerplate (ej. derive de un trait).
97. **Simulador de máquina de Turing** o autómata finito.
98. **Resolvedor de laberintos** con algoritmos de búsqueda (BFS/DFS/A*).
99. **Compresor/descompresor** de archivos usando un algoritmo simple (RLE o Huffman).
100. **Benchmark de concurrencia** comparando `std::thread`, `tokio` y `rayon` en una tarea común.

### 

