//! Transporte TCP real sobre localhost.
//!
//! * `Servidor::iniciar` abre un `TcpListener` y atiende cada conexión en un
//!   hilo, compartiendo el `Nodo` detrás de un `Arc<Mutex<_>>`.
//! * `sincronizar_con` actúa como cliente: saluda, y responde a los mensajes
//!   del par hasta que la conversación se agota.

use crate::nodo::Nodo;
use crate::protocolo::Mensaje;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Servidor de un nodo escuchando en un puerto local.
pub struct Servidor {
    pub direccion: SocketAddr,
    pub nodo: Arc<Mutex<Nodo>>,
    apagar: Arc<AtomicBool>,
    hilo: Option<JoinHandle<()>>,
}

impl Servidor {
    /// Arranca en `127.0.0.1:0` (puerto libre elegido por el SO).
    pub fn iniciar(nodo: Nodo) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let direccion = listener.local_addr()?;
        // Modo no bloqueante para poder chequear la bandera de apagado.
        listener.set_nonblocking(true)?;
        let nodo = Arc::new(Mutex::new(nodo));
        let apagar = Arc::new(AtomicBool::new(false));
        let (n2, a2) = (nodo.clone(), apagar.clone());
        let hilo = thread::spawn(move || {
            while !a2.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let n3 = n2.clone();
                        thread::spawn(move || {
                            let _ = atender(stream, n3);
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(5)),
                    Err(_) => break,
                }
            }
        });
        Ok(Self { direccion, nodo, apagar, hilo: Some(hilo) })
    }

    pub fn altura(&self) -> u64 {
        self.nodo.lock().unwrap().altura()
    }
}

impl Drop for Servidor {
    fn drop(&mut self) {
        self.apagar.store(true, Ordering::Relaxed);
        if let Some(h) = self.hilo.take() {
            let _ = h.join();
        }
    }
}

/// Atiende una conexión: lee líneas, procesa y responde.
fn atender(stream: TcpStream, nodo: Arc<Mutex<Nodo>>) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut escritor = stream.try_clone()?;
    for linea in BufReader::new(stream).lines() {
        let linea = linea?;
        let Ok(m) = Mensaje::desde_linea(&linea) else { continue };
        let respuestas = nodo.lock().unwrap().procesar(m);
        for r in respuestas {
            escritor.write_all(r.a_linea().as_bytes())?;
        }
        escritor.flush()?;
    }
    Ok(())
}

/// Cliente: conecta con `par` y sincroniza `nodo` hasta que no haya más que decir.
/// Devuelve cuántos mensajes se recibieron.
pub fn sincronizar_con(nodo: &Arc<Mutex<Nodo>>, par: SocketAddr, mi_direccion: Option<String>) -> std::io::Result<usize> {
    let stream = TcpStream::connect_timeout(&par, Duration::from_secs(2))?;
    // Timeout corto: si el par no responde más, la sincronización terminó.
    stream.set_read_timeout(Some(Duration::from_millis(300)))?;
    let mut escritor = stream.try_clone()?;
    let saludo = Mensaje::Hola { altura: nodo.lock().unwrap().altura(), direccion: mi_direccion };
    escritor.write_all(saludo.a_linea().as_bytes())?;
    escritor.write_all(Mensaje::PedirPares.a_linea().as_bytes())?;
    let mut lector = BufReader::new(stream);
    let mut recibidos = 0;
    let mut linea = String::new();
    loop {
        linea.clear();
        match lector.read_line(&mut linea) {
            Ok(0) => break,
            Ok(_) => {
                recibidos += 1;
                let Ok(m) = Mensaje::desde_linea(&linea) else { continue };
                let es_hola = matches!(m, Mensaje::Hola { .. });
                let resp = nodo.lock().unwrap().procesar(m);
                for r in resp {
                    if es_hola && matches!(r, Mensaje::Hola { .. }) {
                        continue; // no re-saludamos
                    }
                    escritor.write_all(r.a_linea().as_bytes())?;
                }
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => break,
            Err(e) => return Err(e),
        }
    }
    Ok(recibidos)
}

/// Difunde un mensaje a una lista de pares (best-effort, ignora fallos).
pub fn difundir(m: &Mensaje, pares: &[SocketAddr]) -> usize {
    pares
        .iter()
        .filter(|p| {
            TcpStream::connect_timeout(p, Duration::from_secs(1))
                .and_then(|mut s| s.write_all(m.a_linea().as_bytes()))
                .is_ok()
        })
        .count()
}
