//! # Auditor de red local — lógica
//!
//! Reúne información de la red de la máquina:
//! - **Interfaces** e IPs (vía `if-addrs`), con máscara → prefijo CIDR,
//!   dirección de red, broadcast y cantidad de hosts ([`Subred`]).
//! - **Clasificación** de cada IP ([`clasificar`]): privada RFC1918, loopback,
//!   link-local, CGNAT, pública, ULA IPv6...
//! - **Gateway por defecto**: parsers puros para `/proc/net/route` (Linux),
//!   `netstat -rn` (macOS/BSD) y `route print` (Windows). Al ser funciones
//!   `&str → Option<Ipv4Addr>`, se testean con textos de ejemplo.
//! - **DNS**: parser de `/etc/resolv.conf`.
//! - **Chequeo de puertos TCP** ([`puerto_abierto`]) con timeout, para un host
//!   que el usuario indique (pensado para su propio gateway/equipos).

use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// Tipo de dirección, en español.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TipoIp {
    Loopback,
    Privada,
    LinkLocal,
    Cgnat,
    UlaIpv6,
    Multicast,
    Publica,
    NoEspecificada,
}

/// Clasifica una IP según los rangos reservados más comunes.
pub fn clasificar(ip: IpAddr) -> TipoIp {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            if v.is_unspecified() {
                TipoIp::NoEspecificada
            } else if v.is_loopback() {
                TipoIp::Loopback
            } else if v.is_private() {
                TipoIp::Privada // 10/8, 172.16/12, 192.168/16
            } else if v.is_link_local() {
                TipoIp::LinkLocal // 169.254/16 (APIPA: sin DHCP)
            } else if o[0] == 100 && (64..128).contains(&o[1]) {
                TipoIp::Cgnat // 100.64/10: NAT del proveedor
            } else if v.is_multicast() {
                TipoIp::Multicast
            } else {
                TipoIp::Publica
            }
        }
        IpAddr::V6(v) => {
            let s = v.segments();
            if v.is_unspecified() {
                TipoIp::NoEspecificada
            } else if v.is_loopback() {
                TipoIp::Loopback
            } else if s[0] & 0xffc0 == 0xfe80 {
                TipoIp::LinkLocal // fe80::/10
            } else if s[0] & 0xfe00 == 0xfc00 {
                TipoIp::UlaIpv6 // fc00::/7 (equivalente a privada)
            } else if v.is_multicast() {
                TipoIp::Multicast
            } else {
                TipoIp::Publica
            }
        }
    }
}

/// Datos de una subred IPv4 derivados de IP + máscara.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Subred {
    pub prefijo: u8,
    pub red: Ipv4Addr,
    pub broadcast: Ipv4Addr,
    /// Hosts utilizables (excluye red y broadcast; /31 y /32 son casos especiales).
    pub hosts: u64,
}

/// Cuenta los bits en 1 de una máscara; `None` si no es contigua (ej. 255.0.255.0).
pub fn prefijo_de_mascara(m: Ipv4Addr) -> Option<u8> {
    let n = u32::from(m);
    let p = n.leading_ones();
    // Una máscara válida tiene todos sus unos al principio.
    (n.checked_shl(p).unwrap_or(0) == 0).then_some(p as u8)
}

/// Calcula la subred de una IPv4.
pub fn subred(ip: Ipv4Addr, prefijo: u8) -> Subred {
    let mascara: u32 = if prefijo == 0 { 0 } else { u32::MAX << (32 - prefijo as u32) };
    let red = u32::from(ip) & mascara;
    let broadcast = red | !mascara;
    let hosts = match prefijo {
        32 => 1,
        31 => 2, // RFC 3021: enlaces punto a punto
        p => (1u64 << (32 - p as u32)) - 2,
    };
    Subred { prefijo, red: red.into(), broadcast: broadcast.into(), hosts }
}

/// Una dirección de una interfaz.
#[derive(Debug, Clone, Serialize)]
pub struct Direccion {
    pub interfaz: String,
    pub ip: IpAddr,
    pub tipo: TipoIp,
    /// Solo IPv4 con máscara conocida.
    pub subred: Option<Subred>,
    /// Prefijo IPv6 si se pudo derivar de la máscara.
    pub prefijo_v6: Option<u8>,
}

/// Lista todas las direcciones de todas las interfaces.
pub fn direcciones() -> std::io::Result<Vec<Direccion>> {
    let mut v: Vec<Direccion> = if_addrs::get_if_addrs()?
        .into_iter()
        .map(|i| {
            let ip = i.ip();
            let (sub, p6) = match &i.addr {
                if_addrs::IfAddr::V4(a) => (prefijo_de_mascara(a.netmask).map(|p| subred(a.ip, p)), None),
                if_addrs::IfAddr::V6(a) => (None, Some(u128::from(a.netmask).leading_ones() as u8)),
            };
            Direccion { interfaz: i.name.clone(), ip, tipo: clasificar(ip), subred: sub, prefijo_v6: p6 }
        })
        .collect();
    v.sort_by(|a, b| a.interfaz.cmp(&b.interfaz).then(a.ip.is_ipv6().cmp(&b.ip.is_ipv6())));
    Ok(v)
}

/// Gateway desde `/proc/net/route` (Linux). Las IPs están en hex little-endian.
///
/// ```text
/// Iface   Destination  Gateway   Flags ...
/// eth0    00000000     0101A8C0  0003  ...   ← destino 0.0.0.0 = ruta por defecto
/// ```
pub fn gateway_linux(proc_net_route: &str) -> Option<(String, Ipv4Addr)> {
    proc_net_route.lines().skip(1).find_map(|l| {
        let c: Vec<&str> = l.split_whitespace().collect();
        if c.len() < 3 || c[1] != "00000000" {
            return None;
        }
        let g = u32::from_str_radix(c[2], 16).ok()?;
        // El kernel imprime los 4 bytes de red leídos como u32 nativo; recuperamos
        // los bytes en memoria con `to_ne_bytes` (portable a big-endian también).
        Some((c[0].to_string(), Ipv4Addr::from(g.to_ne_bytes())))
    })
}

/// Gateway desde la salida de `netstat -rn` (macOS/BSD): línea que empieza con `default`.
pub fn gateway_bsd(netstat: &str) -> Option<(String, Ipv4Addr)> {
    netstat.lines().find_map(|l| {
        let c: Vec<&str> = l.split_whitespace().collect();
        if c.first() != Some(&"default") {
            return None;
        }
        let ip: Ipv4Addr = c.get(1)?.parse().ok()?;
        Some((c.last().unwrap_or(&"?").to_string(), ip))
    })
}

/// Gateway desde `route print -4` (Windows): fila `0.0.0.0  0.0.0.0  <gateway>  <interfaz> <métrica>`.
pub fn gateway_windows(route_print: &str) -> Option<(String, Ipv4Addr)> {
    route_print.lines().find_map(|l| {
        let c: Vec<&str> = l.split_whitespace().collect();
        if c.len() >= 4 && c[0] == "0.0.0.0" && c[1] == "0.0.0.0" {
            Some((c[3].to_string(), c[2].parse().ok()?))
        } else {
            None
        }
    })
}

/// Detecta el gateway en el SO actual.
pub fn gateway() -> Option<(String, Ipv4Addr)> {
    #[cfg(target_os = "linux")]
    {
        gateway_linux(&std::fs::read_to_string("/proc/net/route").ok()?)
    }
    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
    {
        let o = std::process::Command::new("netstat").args(["-rn", "-f", "inet"]).output().ok()?;
        gateway_bsd(&String::from_utf8_lossy(&o.stdout))
    }
    #[cfg(windows)]
    {
        let o = std::process::Command::new("route").args(["print", "-4"]).output().ok()?;
        gateway_windows(&String::from_utf8_lossy(&o.stdout))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "freebsd", target_os = "openbsd", windows)))]
    {
        None
    }
}

/// Servidores DNS de un `resolv.conf`.
pub fn dns_de_resolv(texto: &str) -> Vec<IpAddr> {
    texto
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter_map(|l| l.strip_prefix("nameserver"))
        .filter_map(|ip| ip.trim().parse().ok())
        .collect()
}

/// DNS del sistema (solo Unix, vía /etc/resolv.conf).
pub fn dns() -> Vec<IpAddr> {
    std::fs::read_to_string("/etc/resolv.conf").map(|t| dns_de_resolv(&t)).unwrap_or_default()
}

/// Intenta conectar por TCP con timeout. `true` = puerto abierto.
pub fn puerto_abierto(host: IpAddr, puerto: u16, timeout: Duration) -> bool {
    TcpStream::connect_timeout(&SocketAddr::new(host, puerto), timeout).is_ok()
}

/// ¿Es IPv6 global? Utilidad para el resumen.
pub fn es_global_v6(ip: &Ipv6Addr) -> bool {
    clasificar(IpAddr::V6(*ip)) == TipoIp::Publica
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clasifica_rangos() {
        let c = |s: &str| clasificar(s.parse().unwrap());
        assert_eq!(c("192.168.1.10"), TipoIp::Privada);
        assert_eq!(c("10.0.0.1"), TipoIp::Privada);
        assert_eq!(c("172.20.0.1"), TipoIp::Privada);
        assert_eq!(c("127.0.0.1"), TipoIp::Loopback);
        assert_eq!(c("169.254.3.4"), TipoIp::LinkLocal);
        assert_eq!(c("100.70.0.1"), TipoIp::Cgnat);
        assert_eq!(c("8.8.8.8"), TipoIp::Publica);
        assert_eq!(c("fe80::1"), TipoIp::LinkLocal);
        assert_eq!(c("fd00::1"), TipoIp::UlaIpv6);
        assert_eq!(c("::1"), TipoIp::Loopback);
        assert_eq!(c("2001:4860::8888"), TipoIp::Publica);
    }

    #[test]
    fn mascaras_y_subredes() {
        assert_eq!(prefijo_de_mascara("255.255.255.0".parse().unwrap()), Some(24));
        assert_eq!(prefijo_de_mascara("255.255.240.0".parse().unwrap()), Some(20));
        assert_eq!(prefijo_de_mascara("0.0.0.0".parse().unwrap()), Some(0));
        assert_eq!(prefijo_de_mascara("255.0.255.0".parse().unwrap()), None);
        let s = subred("192.168.1.77".parse().unwrap(), 24);
        assert_eq!(s.red, Ipv4Addr::new(192, 168, 1, 0));
        assert_eq!(s.broadcast, Ipv4Addr::new(192, 168, 1, 255));
        assert_eq!(s.hosts, 254);
        assert_eq!(subred("10.0.0.1".parse().unwrap(), 31).hosts, 2);
        assert_eq!(subred("10.0.0.1".parse().unwrap(), 32).hosts, 1);
    }

    #[test]
    fn parsea_gateway_linux() {
        let t = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\n\
                 eth0\t0000A8C0\t00000000\t0001\t0\t0\t0\t00FFFFFF\n\
                 eth0\t00000000\t0101A8C0\t0003\t0\t0\t100\t00000000\n";
        #[cfg(target_endian = "little")]
        assert_eq!(gateway_linux(t), Some(("eth0".into(), Ipv4Addr::new(192, 168, 1, 1))));
        assert_eq!(gateway_linux("Iface\n"), None);
    }

    #[test]
    fn parsea_gateway_bsd_y_windows() {
        let mac = "Routing tables\n\nInternet:\nDestination        Gateway            Flags        Netif Expire\ndefault            192.168.0.1        UGScg          en0\n";
        assert_eq!(gateway_bsd(mac), Some(("en0".into(), Ipv4Addr::new(192, 168, 0, 1))));
        let win = "IPv4 Route Table\n===\nActive Routes:\nNetwork Destination        Netmask          Gateway       Interface  Metric\n          0.0.0.0          0.0.0.0      10.0.0.1     10.0.0.23     25\n";
        assert_eq!(gateway_windows(win), Some(("10.0.0.23".into(), Ipv4Addr::new(10, 0, 0, 1))));
    }

    #[test]
    fn parsea_resolv() {
        let t = "# comentario\nnameserver 1.1.1.1\nnameserver 2606:4700:4700::1111 # cf\nsearch lan\nnameserver basura\n";
        assert_eq!(dns_de_resolv(t).len(), 2);
    }

    #[test]
    fn direcciones_reales_incluyen_loopback() {
        let v = direcciones().unwrap();
        assert!(v.iter().any(|d| d.tipo == TipoIp::Loopback));
    }

    #[test]
    fn chequeo_de_puertos() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        let lo: IpAddr = "127.0.0.1".parse().unwrap();
        assert!(puerto_abierto(lo, p, Duration::from_millis(500)));
        drop(l);
        assert!(!puerto_abierto(lo, p, Duration::from_millis(500)));
    }
}
