//! Lógica del nodo, **independiente del transporte**.
//!
//! `Nodo::procesar` recibe un mensaje y devuelve las respuestas a enviar. Esto
//! permite testear el protocolo en memoria (sin sockets) y reutilizar la misma
//! lógica desde el servidor TCP (`red.rs`).

use crate::cadena::{cadena_valida, Bloque};
use crate::protocolo::Mensaje;
use std::collections::BTreeSet;

/// Máximo de bloques por respuesta, para no mandar mensajes gigantes.
pub const MAX_LOTE: usize = 500;

#[derive(Debug, Clone)]
pub struct Nodo {
    /// Identificador legible (para logs).
    pub nombre: String,
    /// Cadena local; siempre arranca con el génesis común.
    pub bloques: Vec<Bloque>,
    /// Pares conocidos (direcciones `ip:puerto`), ordenados y sin duplicados.
    pub pares: BTreeSet<String>,
    /// Bloques "huérfanos": llegaron antes que su padre; se reintentan luego.
    pub huerfanos: Vec<Bloque>,
}

/// Resultado de recibir un bloque anunciado.
#[derive(Debug, PartialEq, Eq)]
pub enum Recepcion {
    /// Se agregó a la punta de la cadena.
    Agregado,
    /// Ya lo teníamos.
    Duplicado,
    /// Está más adelante: nos faltan bloques intermedios (hay que sincronizar).
    Adelantado,
    /// Inválido o en una rama más corta: se descarta.
    Rechazado,
}

impl Nodo {
    pub fn nuevo(nombre: impl Into<String>) -> Self {
        Self { nombre: nombre.into(), bloques: vec![Bloque::genesis()], pares: BTreeSet::new(), huerfanos: vec![] }
    }

    /// Altura = índice del último bloque.
    pub fn altura(&self) -> u64 {
        self.bloques.last().map(|b| b.indice).unwrap_or(0)
    }

    /// Mina un bloque local con `datos` y lo agrega. Devuelve el anuncio a difundir.
    pub fn minar(&mut self, datos: impl Into<String>, timestamp: u64) -> Mensaje {
        let previo = self.bloques.last().expect("hay génesis");
        let mut b = Bloque { indice: previo.indice + 1, timestamp, datos: datos.into(), hash_previo: previo.hash.clone(), nonce: 0, hash: String::new() };
        b.minar();
        self.bloques.push(b.clone());
        Mensaje::NuevoBloque(b)
    }

    /// Intenta agregar un bloque recibido.
    pub fn recibir_bloque(&mut self, b: Bloque) -> Recepcion {
        if !b.es_valido() {
            return Recepcion::Rechazado;
        }
        let altura = self.altura();
        if b.indice <= altura {
            return if self.bloques[b.indice as usize] == b { Recepcion::Duplicado } else { Recepcion::Rechazado };
        }
        if b.indice == altura + 1 && b.sigue_a(self.bloques.last().unwrap()) {
            self.bloques.push(b);
            self.reintentar_huerfanos();
            return Recepcion::Agregado;
        }
        // Llegó desordenado o somos nosotros los atrasados.
        if !self.huerfanos.contains(&b) {
            self.huerfanos.push(b);
        }
        Recepcion::Adelantado
    }

    /// Conecta huérfanos que ahora sí encajan en la punta.
    fn reintentar_huerfanos(&mut self) {
        loop {
            let punta = self.bloques.last().unwrap().clone();
            match self.huerfanos.iter().position(|h| h.sigue_a(&punta)) {
                Some(i) => {
                    let h = self.huerfanos.remove(i);
                    self.bloques.push(h);
                }
                None => break,
            }
        }
        let altura = self.altura();
        self.huerfanos.retain(|h| h.indice > altura);
    }

    /// Recibe un tramo de bloques: si la cadena resultante es válida y más
    /// larga, la adopta (regla de la cadena más larga). Soporta reorganizaciones.
    pub fn recibir_tramo(&mut self, tramo: Vec<Bloque>) -> bool {
        let Some(primero) = tramo.first() else { return false };
        let desde = primero.indice as usize;
        if desde == 0 || desde > self.bloques.len() {
            // Si el tramo trae el génesis, lo evaluamos como cadena completa.
            if desde == 0 && cadena_valida(&tramo) && tramo.len() > self.bloques.len() {
                self.bloques = tramo;
                self.reintentar_huerfanos();
                return true;
            }
            return false;
        }
        let mut candidata: Vec<Bloque> = self.bloques[..desde].to_vec();
        candidata.extend(tramo);
        if candidata.len() > self.bloques.len() && cadena_valida(&candidata) {
            self.bloques = candidata;
            self.reintentar_huerfanos();
            true
        } else {
            false
        }
    }

    /// Núcleo del protocolo: procesa un mensaje entrante y devuelve respuestas.
    pub fn procesar(&mut self, m: Mensaje) -> Vec<Mensaje> {
        match m {
            Mensaje::Hola { altura, direccion } => {
                if let Some(d) = direccion {
                    self.pares.insert(d);
                }
                let mut r = vec![Mensaje::Hola { altura: self.altura(), direccion: None }];
                if altura > self.altura() {
                    // El otro va adelante: pedimos desde nuestra punta (incluida,
                    // para detectar bifurcaciones en el último bloque).
                    r.push(Mensaje::PedirBloques { desde: self.altura() });
                }
                r
            }
            Mensaje::PedirBloques { desde } => {
                let d = (desde as usize).min(self.bloques.len());
                vec![Mensaje::Bloques(self.bloques[d..].iter().take(MAX_LOTE).cloned().collect())]
            }
            Mensaje::Bloques(tramo) => {
                let ultimo = tramo.last().map(|b| b.indice).unwrap_or(0);
                if self.recibir_tramo(tramo) {
                    vec![]
                } else if ultimo > self.altura() {
                    // No encajó: pedimos la cadena entera para resolver la bifurcación.
                    vec![Mensaje::PedirBloques { desde: 0 }]
                } else {
                    vec![]
                }
            }
            Mensaje::NuevoBloque(b) => match self.recibir_bloque(b) {
                Recepcion::Adelantado => vec![Mensaje::PedirBloques { desde: self.altura() }],
                _ => vec![],
            },
            Mensaje::PedirPares => vec![Mensaje::Pares(self.pares.iter().cloned().collect())],
            Mensaje::Pares(lista) => {
                self.pares.extend(lista);
                vec![]
            }
        }
    }
}

/// Simula una conversación completa entre dos nodos en memoria hasta que no
/// queden mensajes pendientes. Devuelve la cantidad de mensajes intercambiados.
pub fn sincronizar_en_memoria(a: &mut Nodo, b: &mut Nodo) -> usize {
    // Cola de (destino_es_b, mensaje).
    let mut cola: std::collections::VecDeque<(bool, Mensaje)> = std::collections::VecDeque::new();
    cola.push_back((true, Mensaje::Hola { altura: a.altura(), direccion: None }));
    let mut n = 0;
    while let Some((a_b, m)) = cola.pop_front() {
        n += 1;
        assert!(n < 10_000, "protocolo en bucle");
        let (destino, de_vuelta) = if a_b { (&mut *b, false) } else { (&mut *a, true) };
        for r in destino.procesar(m) {
            // Evitamos ping-pong infinito de saludos.
            if matches!(r, Mensaje::Hola { .. }) && n > 1 {
                continue;
            }
            cola.push_back((de_vuelta, r));
        }
    }
    n
}
