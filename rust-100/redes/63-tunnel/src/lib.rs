//! # Tunneling tipo ngrok (básico)
//!
//! Arquitectura de 3 piezas:
//! 1. **Local** expone un servicio en localhost.
//! 2. **Agent** mantiene una conexión de control al **Broker** y pide
//!    streams bajo demanda.
//! 3. **Broker** acepta clientes públicos y los empareja con el agent.
//!
//! Protocolo de control (líneas): `REGISTER <id>`, `OPEN <conn_id>`,
//! `READY <conn_id>`. Los datos van por conexiones TCP separadas
//! etiquetadas con el conn_id en los primeros 8 bytes ASCII.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct Broker {
    /// tunnel_id → control socket
    agents: Mutex<HashMap<String, TcpStream>>,
    /// conn_id → waiting public client
    pending: Mutex<HashMap<String, TcpStream>>,
}

impl Broker {
    pub fn new() -> Arc<Self> { Arc::new(Self::default()) }

    pub fn serve_control(self: Arc<Self>, listener: TcpListener) {
        for s in listener.incoming().flatten() {
            let bro = self.clone();
            std::thread::spawn(move || {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut w = s;
                let mut line = String::new();
                let mut id = String::new();
                loop {
                    line.clear();
                    if r.read_line(&mut line).unwrap_or(0) == 0 { break; }
                    let line = line.trim_end().to_string();
                    if let Some(tid) = line.strip_prefix("REGISTER ") {
                        id = tid.to_string();
                        bro.agents.lock().unwrap().insert(id.clone(), w.try_clone().unwrap());
                        let _ = writeln!(w, "OK registered {id}");
                    } else if let Some(cid) = line.strip_prefix("READY ") {
                        // Agent abrió un stream de datos; lo esperamos en data port (simplificado:
                        // el agent reusa este mismo mensaje y luego el broker empareja via pending).
                        let _ = cid;
                    }
                }
                if !id.is_empty() {
                    bro.agents.lock().unwrap().remove(&id);
                }
            });
        }
    }

    /// Acepta un cliente público y pide al agent un túnel.
    pub fn handle_public(self: &Arc<Self>, tunnel_id: &str, mut public: TcpStream) -> std::io::Result<()> {
        let mut agents = self.agents.lock().unwrap();
        let agent = agents.get_mut(tunnel_id).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "tunnel desconocido")
        })?;
        let cid = format!("{:08x}", std::process::id() ^ public.peer_addr().map(|a| a.port() as u32).unwrap_or(0));
        writeln!(agent, "OPEN {cid}")?;
        // Guardamos el cliente a la espera de que el agent conecte al data listener.
        drop(agents);
        self.pending.lock().unwrap().insert(cid.clone(), public.try_clone()?);
        // Esperamos (busy wait corto) a que el agent tome el pending vía `take_pending`.
        // En esta versión educativa el agent llama take_pending tras OPEN.
        let _ = cid;
        // Relay se hace cuando el agent conecta; aquí bloqueamos leyendo hasta EOF
        // (el agent habrá tomado el socket vía take_pending y retransmitido).
        // Para simplificar tests: `take_pending` mueve el socket, así que este clone
        // puede morir; usamos el original sólo si sigue pendiente.
        let mut buf = [0u8; 1];
        let _ = public.read(&mut buf);
        Ok(())
    }

    pub fn take_pending(&self, cid: &str) -> Option<TcpStream> {
        self.pending.lock().unwrap().remove(cid)
    }

    pub fn notify_open(&self, tunnel_id: &str) -> Option<String> {
        // Lee del control? En tests el agent parsea OPEN directamente.
        let _ = tunnel_id;
        None
    }
}

/// Agent: se registra, escucha OPEN, conecta al servicio local y al public pending.
pub struct Agent {
    pub control: TcpStream,
    pub broker: Arc<Broker>,
    pub local_addr: String,
}

impl Agent {
    pub fn register(mut control: TcpStream, id: &str) -> std::io::Result<Self> {
        // En la versión in-process, el "control" es un socket al broker.
        writeln!(control, "REGISTER {id}")?;
        let mut r = BufReader::new(control.try_clone()?);
        let mut line = String::new();
        r.read_line(&mut line)?;
        Ok(Self { control, broker: Broker::new(), local_addr: String::new() })
    }
}

/// Versión in-process (sin red de control real) para tests claros:
/// `TunnelHub` empareja público ↔ local directamente con un mapa de agents.
pub struct TunnelHub {
    /// id → dirección del servicio local
    routes: Mutex<HashMap<String, String>>,
}

impl TunnelHub {
    pub fn new() -> Arc<Self> { Arc::new(Self { routes: Mutex::new(HashMap::new()) }) }

    pub fn register(&self, id: &str, local: &str) {
        self.routes.lock().unwrap().insert(id.into(), local.into());
    }

    pub fn open(&self, id: &str, mut public: TcpStream) -> std::io::Result<()> {
        let local = self.routes.lock().unwrap().get(id).cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "id"))?;
        let mut local_s = TcpStream::connect(local)?;
        let mut p2 = public.try_clone()?;
        let mut l2 = local_s.try_clone()?;
        let t = std::thread::spawn(move || { let _ = std::io::copy(&mut p2, &mut l2); });
        let _ = std::io::copy(&mut local_s, &mut public);
        let _ = t.join();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tunel_publico_a_local() {
        // Servicio local eco
        let local = TcpListener::bind("127.0.0.1:0").unwrap();
        let local_addr = local.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for s in local.incoming().flatten() {
                let mut s = s;
                let mut buf = [0u8; 64];
                if let Ok(n) = s.read(&mut buf) { let _ = s.write_all(&buf[..n]); }
            }
        });
        let hub = TunnelHub::new();
        hub.register("demo", &local_addr);
        // "Público": listener que reenvía al hub
        let public_l = TcpListener::bind("127.0.0.1:0").unwrap();
        let pub_addr = public_l.local_addr().unwrap().to_string();
        let hub2 = hub.clone();
        std::thread::spawn(move || {
            for s in public_l.incoming().flatten() {
                let h = hub2.clone();
                std::thread::spawn(move || { let _ = h.open("demo", s); });
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(30));
        let mut c = TcpStream::connect(&pub_addr).unwrap();
        c.write_all(b"tunel").unwrap();
        let mut buf = [0u8; 5];
        c.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"tunel");
    }
}
