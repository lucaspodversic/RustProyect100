//! Servidor autoritativo y cliente TCP.
//!
//! Arquitectura del servidor:
//! - acepta 2 conexiones; cada una tiene un hilo lector que convierte líneas
//!   en mensajes y los manda por un canal `mpsc` al bucle principal;
//! - el bucle principal corre a tick fijo: vacía el canal, aplica la última
//!   entrada de cada jugador, simula y difunde `ESTADO` a ambos;
//! - conexiones extra reciben `ERROR lleno`; si un jugador se desconecta,
//!   gana el otro.
//!
//! El servidor es la única fuente de verdad: los clientes sólo mandan
//! intenciones (arriba/abajo), nunca posiciones (así no se puede hacer trampa).

use crate::juego::{ia, Estado, Juego};
use crate::protocolo::{MsgCliente, MsgServidor, MAX_LINEA};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Config {
    pub para_ganar: u32,
    /// Paso de simulación (s).
    pub dt: f32,
    /// Duración real de cada tick (0 = lo más rápido posible, para tests).
    pub tick_real: Duration,
    /// Corte de seguridad: al llegar, gana quien va adelante (o empate).
    pub max_ticks: u64,
    /// Cada cuántos ticks se envía el estado.
    pub enviar_cada: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self { para_ganar: 5, dt: 1.0 / 60.0, tick_real: Duration::from_millis(16), max_ticks: 60 * 60 * 10, enviar_cada: 1 }
    }
}

#[derive(Debug)]
enum Aviso {
    Msg(usize, MsgCliente),
    Desconectado(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resultado {
    pub ganador: Option<usize>,
    pub puntos: [u32; 2],
    pub nombres: [String; 2],
    pub ticks: u64,
    pub por_abandono: bool,
}

/// Lee una línea con límite de tamaño (evita que un cliente malicioso
/// llene la memoria mandando bytes sin `\n`).
fn leer_linea<R: BufRead>(r: &mut R, buf: &mut String) -> std::io::Result<usize> {
    buf.clear();
    let n = r.by_ref().take(MAX_LINEA as u64 + 1).read_line(buf)?;
    if n > MAX_LINEA {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "línea demasiado larga"));
    }
    Ok(n)
}

fn enviar(s: &mut Option<TcpStream>, m: &MsgServidor) {
    if let Some(st) = s {
        if st.write_all(m.a_linea().as_bytes()).is_err() {
            *s = None;
        }
    }
}

/// Ejecuta una partida completa sobre un listener ya ligado.
pub fn servir(listener: TcpListener, cfg: Config) -> std::io::Result<Resultado> {
    let (tx, rx) = mpsc::channel::<Aviso>();
    let mut escritores: [Option<TcpStream>; 2] = [None, None];
    let mut nombres = [String::from("?"), String::from("?")];
    // 1) Sala de espera: aceptar 2 jugadores y su HOLA.
    for (j, esc) in escritores.iter_mut().enumerate() {
        let (stream, _) = listener.accept()?;
        stream.set_nodelay(true)?;
        let mut lector = BufReader::new(stream.try_clone()?);
        let mut l = String::new();
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        leer_linea(&mut lector, &mut l)?;
        stream.set_read_timeout(None)?;
        nombres[j] = match MsgCliente::parsear(&l) {
            Ok(MsgCliente::Hola(n)) => n,
            _ => "anon".into(),
        };
        let mut s = Some(stream);
        enviar(&mut s, &MsgServidor::Bienvenido(j));
        *esc = s;
        let tx = tx.clone();
        thread::spawn(move || {
            let mut l = String::new();
            loop {
                match leer_linea(&mut lector, &mut l) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => match MsgCliente::parsear(&l) {
                        Ok(MsgCliente::Chau) => break,
                        Ok(m) => {
                            if tx.send(Aviso::Msg(j, m)).is_err() {
                                return;
                            }
                        }
                        Err(_) => {} // ignoramos basura, no tumba la partida
                    },
                }
            }
            let _ = tx.send(Aviso::Desconectado(j));
        });
    }
    enviar(&mut escritores[0], &MsgServidor::Inicio(nombres[1].clone()));
    enviar(&mut escritores[1], &MsgServidor::Inicio(nombres[0].clone()));
    listener.set_nonblocking(true)?;

    // 2) Bucle de juego.
    let mut juego = Juego::new(cfg.para_ganar);
    let mut entradas = [0i8; 2];
    let mut abandono = None;
    let mut proximo = Instant::now();
    while juego.estado.ganador.is_none() && juego.estado.tick < cfg.max_ticks {
        // Rechazar espectadores/terceros.
        while let Ok((mut extra, _)) = listener.accept() {
            let _ = extra.set_nonblocking(false);
            let _ = extra.write_all(MsgServidor::Error("lleno".into()).a_linea().as_bytes());
        }
        while let Ok(a) = rx.try_recv() {
            match a {
                Aviso::Msg(j, MsgCliente::Entrada(d)) => entradas[j] = d,
                Aviso::Msg(..) => {}
                Aviso::Desconectado(j) => abandono = Some(j),
            }
        }
        if let Some(j) = abandono {
            juego.estado.ganador = Some(1 - j);
            break;
        }
        juego.paso(entradas, cfg.dt);
        if juego.estado.tick % cfg.enviar_cada == 0 {
            let m = MsgServidor::Estado(juego.estado.clone());
            escritores.iter_mut().for_each(|e| enviar(e, &m));
        }
        if !cfg.tick_real.is_zero() {
            proximo += cfg.tick_real;
            if let Some(espera) = proximo.checked_duration_since(Instant::now()) {
                thread::sleep(espera);
            }
        }
    }
    let e = &juego.estado;
    let ganador = e.ganador.or(match e.puntos[0].cmp(&e.puntos[1]) {
        std::cmp::Ordering::Greater => Some(0),
        std::cmp::Ordering::Less => Some(1),
        std::cmp::Ordering::Equal => None,
    });
    escritores.iter_mut().for_each(|w| enviar(w, &MsgServidor::Fin(ganador)));
    Ok(Resultado { ganador, puntos: e.puntos, nombres, ticks: e.tick, por_abandono: abandono.is_some() })
}

/// Cómo decide un cliente su entrada a partir del último estado.
pub type Controlador = Box<dyn FnMut(&Estado, usize) -> i8 + Send>;

pub fn controlador_ia(zona_muerta: f32) -> Controlador {
    Box::new(move |e, j| ia(e, j, zona_muerta))
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResumenCliente {
    pub jugador: usize,
    pub rival: String,
    pub estados: u64,
    pub ultimo: Option<Estado>,
    pub fin: Option<Option<usize>>,
}

/// Cliente: se conecta, saluda y responde a cada `ESTADO` con su entrada
/// (sólo envía cuando cambia, para no saturar). `al_estado` permite dibujar.
pub fn jugar(dir: &str, nombre: &str, mut ctl: Controlador, mut al_estado: impl FnMut(&Estado)) -> std::io::Result<ResumenCliente> {
    let mut s = TcpStream::connect(dir)?;
    s.set_nodelay(true)?;
    s.write_all(MsgCliente::Hola(nombre.into()).a_linea().as_bytes())?;
    let mut r = BufReader::new(s.try_clone()?);
    let mut res = ResumenCliente { jugador: 0, rival: String::new(), estados: 0, ultimo: None, fin: None };
    let mut ultima = 0i8;
    let mut l = String::new();
    while leer_linea(&mut r, &mut l)? > 0 {
        match MsgServidor::parsear(&l) {
            Ok(MsgServidor::Bienvenido(j)) => res.jugador = j,
            Ok(MsgServidor::Inicio(rv)) => res.rival = rv,
            Ok(MsgServidor::Estado(e)) => {
                res.estados += 1;
                let d = ctl(&e, res.jugador);
                if d != ultima {
                    ultima = d;
                    if s.write_all(MsgCliente::Entrada(d).a_linea().as_bytes()).is_err() {
                        break;
                    }
                }
                al_estado(&e);
                res.ultimo = Some(e);
            }
            Ok(MsgServidor::Fin(g)) => {
                res.fin = Some(g);
                break;
            }
            Ok(MsgServidor::Error(m)) => return Err(std::io::Error::other(m)),
            Err(_) => {}
        }
    }
    let _ = s.write_all(MsgCliente::Chau.a_linea().as_bytes());
    Ok(res)
}
