//! # Servidor DNS simple
//!
//! Resuelve A/AAAA desde un archivo de zona. Habla DNS UDP wire format
//! (subset: header + QNAME + QTYPE/QCLASS + respuesta con un RR).

use std::collections::HashMap;
use std::net::UdpSocket;

#[derive(Debug, Clone)]
pub struct Zone {
    /// nombre (FQDN lowercase, con punto final) → lista de IPv4
    pub a: HashMap<String, Vec<[u8; 4]>>,
    pub aaaa: HashMap<String, Vec<[u8; 16]>>,
}

impl Zone {
    pub fn parse(text: &str) -> Self {
        let mut a = HashMap::new();
        let mut aaaa = HashMap::new();
        for line in text.lines() {
            let line = line.split('#').next().unwrap().trim();
            if line.is_empty() { continue; }
            let p: Vec<_> = line.split_whitespace().collect();
            if p.len() < 3 { continue; }
            let name = normalize(p[0]);
            match p[1].to_uppercase().as_str() {
                "A" => {
                    if let Ok(ip) = parse_v4(p[2]) {
                        a.entry(name).or_insert_with(Vec::new).push(ip);
                    }
                }
                "AAAA" => {
                    if let Ok(ip) = parse_v6(p[2]) {
                        aaaa.entry(name).or_insert_with(Vec::new).push(ip);
                    }
                }
                _ => {}
            }
        }
        Self { a, aaaa }
    }
}

fn normalize(n: &str) -> String {
    let mut s = n.to_ascii_lowercase();
    if !s.ends_with('.') { s.push('.'); }
    s
}

fn parse_v4(s: &str) -> Result<[u8; 4], ()> {
    let p: Vec<_> = s.split('.').collect();
    if p.len() != 4 { return Err(()); }
    let mut out = [0u8; 4];
    for i in 0..4 {
        out[i] = p[i].parse().map_err(|_| ())?;
    }
    Ok(out)
}

fn parse_v6(s: &str) -> Result<[u8; 16], ()> {
    // Sólo forma completa con 8 grupos hex (educativo).
    let p: Vec<_> = s.split(':').collect();
    if p.len() != 8 { return Err(()); }
    let mut out = [0u8; 16];
    for i in 0..8 {
        let v = u16::from_str_radix(p[i], 16).map_err(|_| ())?;
        out[i * 2] = (v >> 8) as u8;
        out[i * 2 + 1] = v as u8;
    }
    Ok(out)
}

/// Codifica un QNAME: labels length-prefixed + 0.
pub fn encode_name(name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for label in name.trim_end_matches('.').split('.') {
        out.push(label.len() as u8);
        out.extend(label.as_bytes());
    }
    out.push(0);
    out
}

pub fn decode_name(buf: &[u8], mut i: usize) -> Result<(String, usize), String> {
    let mut labels = Vec::new();
    let mut jumped = false;
    let mut return_i = i;
    let mut hops = 0;
    loop {
        if i >= buf.len() { return Err("truncado".into()); }
        let len = buf[i];
        if len == 0 {
            if !jumped { return_i = i + 1; }
            break;
        }
        if len & 0xC0 == 0xC0 {
            // puntero de compresión
            if i + 1 >= buf.len() { return Err("ptr".into()); }
            let off = (((len as usize) & 0x3F) << 8) | buf[i + 1] as usize;
            if !jumped { return_i = i + 2; jumped = true; }
            i = off;
            hops += 1;
            if hops > 10 { return Err("loop".into()); }
            continue;
        }
        i += 1;
        if i + len as usize > buf.len() { return Err("label".into()); }
        labels.push(String::from_utf8_lossy(&buf[i..i + len as usize]).to_string());
        i += len as usize;
        if !jumped { return_i = i; }
    }
    let mut name = labels.join(".");
    name.push('.');
    Ok((name.to_ascii_lowercase(), return_i))
}

/// Construye una respuesta a partir de un query crudo.
pub fn answer(zone: &Zone, query: &[u8]) -> Result<Vec<u8>, String> {
    if query.len() < 12 { return Err("corto".into()); }
    let id = &query[0..2];
    let qcount = u16::from_be_bytes([query[4], query[5]]);
    if qcount != 1 { return Err("sólo 1 pregunta".into()); }
    let (name, mut i) = decode_name(query, 12)?;
    if i + 4 > query.len() { return Err("sin qtype".into()); }
    let qtype = u16::from_be_bytes([query[i], query[i + 1]]);
    let qclass = u16::from_be_bytes([query[i + 2], query[i + 3]]);
    i += 4;

    let mut answers: Vec<Vec<u8>> = Vec::new();
    match qtype {
        1 => { // A
            for ip in zone.a.get(&name).into_iter().flatten() {
                let mut rr = encode_name(&name);
                rr.extend_from_slice(&1u16.to_be_bytes());
                rr.extend_from_slice(&qclass.to_be_bytes());
                rr.extend_from_slice(&60u32.to_be_bytes()); // TTL
                rr.extend_from_slice(&4u16.to_be_bytes());
                rr.extend_from_slice(ip);
                answers.push(rr);
            }
        }
        28 => { // AAAA
            for ip in zone.aaaa.get(&name).into_iter().flatten() {
                let mut rr = encode_name(&name);
                rr.extend_from_slice(&28u16.to_be_bytes());
                rr.extend_from_slice(&qclass.to_be_bytes());
                rr.extend_from_slice(&60u32.to_be_bytes());
                rr.extend_from_slice(&16u16.to_be_bytes());
                rr.extend_from_slice(ip);
                answers.push(rr);
            }
        }
        _ => {}
    }
    let rcode: u8 = if answers.is_empty() { 3 } else { 0 }; // NXDOMAIN o NOERROR
    let mut out = Vec::new();
    out.extend_from_slice(id);
    // flags: QR=1, AA=1, RCODE
    let flags: u16 = 0x8400 | rcode as u16;
    out.extend_from_slice(&flags.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
    out.extend_from_slice(&(answers.len() as u16).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // NS
    out.extend_from_slice(&0u16.to_be_bytes()); // AR
    // reusar la pregunta original
    out.extend_from_slice(&query[12..i]);
    for rr in answers { out.extend(rr); }
    Ok(out)
}

pub fn serve(sock: UdpSocket, zone: Zone) {
    let mut buf = [0u8; 512];
    loop {
        if let Ok((n, src)) = sock.recv_from(&mut buf) {
            if let Ok(resp) = answer(&zone, &buf[..n]) {
                let _ = sock.send_to(&resp, src);
            }
        }
    }
}

/// Cliente: arma un query A y parsea la primera IP.
pub fn query_a(sock: &UdpSocket, server: &str, name: &str) -> Result<Vec<[u8; 4]>, String> {
    let mut q = Vec::new();
    q.extend_from_slice(&0x1234u16.to_be_bytes());
    q.extend_from_slice(&0x0100u16.to_be_bytes()); // RD
    q.extend_from_slice(&1u16.to_be_bytes());
    q.extend_from_slice(&0u16.to_be_bytes());
    q.extend_from_slice(&0u16.to_be_bytes());
    q.extend_from_slice(&0u16.to_be_bytes());
    q.extend(encode_name(name));
    q.extend_from_slice(&1u16.to_be_bytes());
    q.extend_from_slice(&1u16.to_be_bytes());
    sock.send_to(&q, server).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 512];
    let (n, _) = sock.recv_from(&mut buf).map_err(|e| e.to_string())?;
    let ancount = u16::from_be_bytes([buf[6], buf[7]]);
    let (_, mut i) = decode_name(&buf, 12)?;
    i += 4; // skip qtype/qclass
    let mut ips = Vec::new();
    for _ in 0..ancount {
        let (_, ni) = decode_name(&buf, i)?;
        i = ni;
        let rtype = u16::from_be_bytes([buf[i], buf[i+1]]);
        i += 8; // type class ttl
        let rdlen = u16::from_be_bytes([buf[i], buf[i+1]]) as usize;
        i += 2;
        if rtype == 1 && rdlen == 4 {
            let mut ip = [0u8; 4];
            ip.copy_from_slice(&buf[i..i+4]);
            ips.push(ip);
        }
        i += rdlen;
    }
    Ok(ips)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zona_y_query() {
        let zone = Zone::parse("example.com. A 1.2.3.4\nexample.com. A 1.2.3.5\nlocalhost. A 127.0.0.1\n");
        let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = sock.local_addr().unwrap().to_string();
        std::thread::spawn(move || serve(sock, zone));
        std::thread::sleep(std::time::Duration::from_millis(30));
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client.set_read_timeout(Some(std::time::Duration::from_millis(500))).unwrap();
        let ips = query_a(&client, &addr, "example.com.").unwrap();
        assert_eq!(ips.len(), 2);
        assert!(ips.contains(&[1,2,3,4]));
        let ips = query_a(&client, &addr, "nope.test.").unwrap();
        assert!(ips.is_empty());
    }
}
