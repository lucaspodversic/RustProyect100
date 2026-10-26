//! # Notificaciones push (WebSockets + colas)
//!
//! Hub in-process: clientes se suscriben a temas; un productor encola
//! mensajes y el hub los entrega a todos los suscriptores del tema.
//! El "WebSocket" aquí es un canal mpsc tipado (mismo patrón asíncrono
//! que un WS real) + un encoder de frames WS texto para demostración.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

/// Frame WebSocket texto (opcode 0x1, mask=0, payload UTF-8).
pub fn ws_text_frame(text: &str) -> Vec<u8> {
    let payload = text.as_bytes();
    let mut out = Vec::new();
    out.push(0x81); // FIN + text
    if payload.len() < 126 {
        out.push(payload.len() as u8);
    } else if payload.len() < 65536 {
        out.push(126);
        out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        out.push(127);
        out.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }
    out.extend_from_slice(payload);
    out
}

pub fn ws_parse_text(frame: &[u8]) -> Option<String> {
    if frame.len() < 2 || frame[0] & 0x0f != 0x1 { return None; }
    let mut i = 2;
    let mut len = (frame[1] & 0x7f) as usize;
    let masked = frame[1] & 0x80 != 0;
    if len == 126 {
        if frame.len() < 4 { return None; }
        len = u16::from_be_bytes([frame[2], frame[3]]) as usize;
        i = 4;
    }
    let mask = if masked {
        if frame.len() < i + 4 { return None; }
        let m = [frame[i], frame[i+1], frame[i+2], frame[i+3]];
        i += 4;
        Some(m)
    } else { None };
    if frame.len() < i + len { return None; }
    let mut data = frame[i..i+len].to_vec();
    if let Some(m) = mask {
        for (j, b) in data.iter_mut().enumerate() { *b ^= m[j % 4]; }
    }
    String::from_utf8(data).ok()
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub topic: String,
    pub body: String,
}

#[derive(Default)]
struct Inner {
    /// topic → lista de colas de suscriptores
    subs: HashMap<String, Vec<usize>>,
    /// id → cola
    queues: HashMap<usize, VecDeque<Notification>>,
    next_id: usize,
}

#[derive(Clone, Default)]
pub struct PushHub {
    inner: Arc<Mutex<Inner>>,
}

impl PushHub {
    pub fn new() -> Self { Self::default() }

    pub fn subscribe(&self, topic: &str) -> usize {
        let mut g = self.inner.lock().unwrap();
        let id = g.next_id;
        g.next_id += 1;
        g.queues.insert(id, VecDeque::new());
        g.subs.entry(topic.into()).or_default().push(id);
        id
    }

    pub fn unsubscribe(&self, id: usize) {
        let mut g = self.inner.lock().unwrap();
        g.queues.remove(&id);
        for list in g.subs.values_mut() {
            list.retain(|&x| x != id);
        }
    }

    pub fn publish(&self, topic: &str, body: impl Into<String>) -> usize {
        let n = Notification { topic: topic.into(), body: body.into() };
        let mut g = self.inner.lock().unwrap();
        let ids = g.subs.get(topic).cloned().unwrap_or_default();
        let mut delivered = 0;
        for id in ids {
            if let Some(q) = g.queues.get_mut(&id) {
                q.push_back(n.clone());
                delivered += 1;
            }
        }
        delivered
    }

    pub fn poll(&self, id: usize) -> Option<Notification> {
        self.inner.lock().unwrap().queues.get_mut(&id)?.pop_front()
    }

    pub fn encode(&self, n: &Notification) -> Vec<u8> {
        ws_text_frame(&format!("{}|{}", n.topic, n.body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pubsub_y_ws_frame() {
        let hub = PushHub::new();
        let a = hub.subscribe("news");
        let b = hub.subscribe("news");
        let c = hub.subscribe("sports");
        assert_eq!(hub.publish("news", "hola"), 2);
        assert_eq!(hub.publish("sports", "gol"), 1);
        assert_eq!(hub.poll(a).unwrap().body, "hola");
        assert_eq!(hub.poll(b).unwrap().body, "hola");
        assert_eq!(hub.poll(c).unwrap().body, "gol");
        assert!(hub.poll(a).is_none());
        let frame = ws_text_frame("ping");
        assert_eq!(ws_parse_text(&frame).unwrap(), "ping");
        // roundtrip con mask
        // construir frame enmascarado a mano
        let payload = b"hola";
        let mask = [1u8, 2, 3, 4];
        let mut m = vec![0x81, 0x80 | 4];
        m.extend_from_slice(&mask);
        for (i, b) in payload.iter().enumerate() { m.push(b ^ mask[i % 4]); }
        assert_eq!(ws_parse_text(&m).unwrap(), "hola");
        hub.unsubscribe(a);
        assert_eq!(hub.publish("news", "x"), 1);
    }
}
