//! # Benchmark HTTP — lógica
//!
//! Implementamos un cliente HTTP/1.1 **mínimo** sobre `tokio::net::TcpStream`
//! (sin reqwest/hyper) para que se vea todo el protocolo y el binario sea liviano:
//!
//! - [`Url`]: parseo de `http://host:puerto/ruta`.
//! - [`Conexion`]: una conexión keep-alive que envía requests y lee la
//!   respuesta respetando `Content-Length` o `Transfer-Encoding: chunked`.
//! - [`correr`]: lanza `concurrencia` workers (tareas tokio) que comparten un
//!   contador atómico de requests restantes; cada uno mide latencias.
//! - [`Informe`]: agrega latencias → percentiles (p50/p90/p99), req/s, códigos.
//!
//! HTTPS queda fuera de alcance (documentado en el README).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// URL HTTP descompuesta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub host: String,
    pub puerto: u16,
    /// Ruta + query, siempre empieza con `/`.
    pub ruta: String,
}

impl Url {
    /// Parsea `http://host[:puerto][/ruta]`. Rechaza https explícitamente.
    pub fn parsear(s: &str) -> Result<Url, String> {
        if s.starts_with("https://") {
            return Err("https no soportado: este benchmark habla HTTP/1.1 plano".into());
        }
        let resto = s.strip_prefix("http://").ok_or("la URL debe empezar con http://")?;
        let (hostport, ruta) = match resto.find('/') {
            Some(i) => (&resto[..i], &resto[i..]),
            None => (resto, "/"),
        };
        let (host, puerto) = match hostport.rsplit_once(':') {
            Some((h, p)) => (h, p.parse::<u16>().map_err(|_| format!("puerto inválido: {p}"))?),
            None => (hostport, 80),
        };
        if host.is_empty() {
            return Err("falta el host".into());
        }
        Ok(Url { host: host.to_string(), puerto, ruta: ruta.to_string() })
    }
}

/// Conexión keep-alive reutilizable.
pub struct Conexion {
    lector: BufReader<TcpStream>,
    url: Url,
}

impl Conexion {
    pub async fn abrir(url: &Url) -> std::io::Result<Self> {
        let s = TcpStream::connect((url.host.as_str(), url.puerto)).await?;
        s.set_nodelay(true)?; // sin Nagle: medimos latencia real de cada request
        Ok(Conexion { lector: BufReader::new(s), url: url.clone() })
    }

    /// Envía un GET y devuelve (código de estado, bytes del cuerpo).
    pub async fn get(&mut self) -> std::io::Result<(u16, usize)> {
        let req = format!("GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: benchmark-http/0.1\r\nAccept: */*\r\n\r\n", self.url.ruta, self.url.host);
        self.lector.get_mut().write_all(req.as_bytes()).await?;
        // Línea de estado: "HTTP/1.1 200 OK"
        let mut linea = String::new();
        if self.lector.read_line(&mut linea).await? == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "el servidor cerró la conexión"));
        }
        let codigo: u16 = linea.split_whitespace().nth(1).and_then(|c| c.parse().ok()).ok_or_else(|| invalido("línea de estado inválida"))?;
        // Cabeceras hasta la línea vacía.
        let mut largo: Option<usize> = None;
        let mut chunked = false;
        loop {
            linea.clear();
            self.lector.read_line(&mut linea).await?;
            let l = linea.trim_end();
            if l.is_empty() {
                break;
            }
            if let Some((k, v)) = l.split_once(':') {
                let k = k.trim().to_ascii_lowercase();
                if k == "content-length" {
                    largo = v.trim().parse().ok();
                } else if k == "transfer-encoding" && v.to_ascii_lowercase().contains("chunked") {
                    chunked = true;
                }
            }
        }
        // Cuerpo: lo leemos y descartamos, contando bytes.
        let bytes = if chunked {
            self.leer_chunked().await?
        } else {
            let n = largo.unwrap_or(0);
            let mut buf = vec![0u8; n];
            self.lector.read_exact(&mut buf).await?;
            n
        };
        Ok((codigo, bytes))
    }

    /// Lee un cuerpo `chunked`: "<tamaño hex>\r\n<datos>\r\n" ... "0\r\n\r\n".
    async fn leer_chunked(&mut self) -> std::io::Result<usize> {
        let mut total = 0;
        let mut l = String::new();
        loop {
            l.clear();
            self.lector.read_line(&mut l).await?;
            let tam = usize::from_str_radix(l.trim().split(';').next().unwrap_or(""), 16).map_err(|_| invalido("chunk inválido"))?;
            let mut buf = vec![0u8; tam + 2]; // + CRLF final del chunk
            self.lector.read_exact(&mut buf).await?;
            if tam == 0 {
                return Ok(total);
            }
            total += tam;
        }
    }
}

fn invalido(m: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, m.to_string())
}

/// Configuración de la corrida.
#[derive(Debug, Clone)]
pub struct Config {
    pub url: Url,
    pub total: u64,
    pub concurrencia: usize,
    pub timeout: Duration,
}

/// Resultado de una request individual.
#[derive(Debug, Clone)]
pub enum Resultado {
    Ok { codigo: u16, latencia: Duration, bytes: usize },
    Error(String),
}

/// Estadísticas agregadas.
#[derive(Debug, Default, Clone)]
pub struct Informe {
    pub total: u64,
    pub errores: u64,
    pub bytes: u64,
    pub duracion: Duration,
    /// Latencias de las requests exitosas, ordenadas.
    pub latencias: Vec<Duration>,
    pub codigos: BTreeMap<u16, u64>,
    pub muestras_error: BTreeMap<String, u64>,
}

impl Informe {
    /// Construye el informe a partir de los resultados y la duración total.
    pub fn desde(resultados: Vec<Resultado>, duracion: Duration) -> Self {
        let mut i = Informe { duracion, ..Default::default() };
        for r in resultados {
            i.total += 1;
            match r {
                Resultado::Ok { codigo, latencia, bytes } => {
                    *i.codigos.entry(codigo).or_default() += 1;
                    i.latencias.push(latencia);
                    i.bytes += bytes as u64;
                }
                Resultado::Error(e) => {
                    i.errores += 1;
                    *i.muestras_error.entry(e).or_default() += 1;
                }
            }
        }
        i.latencias.sort();
        i
    }

    /// Percentil por el método "nearest-rank" (p entre 0 y 100).
    pub fn percentil(&self, p: f64) -> Option<Duration> {
        if self.latencias.is_empty() {
            return None;
        }
        let rango = ((p / 100.0) * self.latencias.len() as f64).ceil() as usize;
        Some(self.latencias[rango.clamp(1, self.latencias.len()) - 1])
    }

    /// Requests por segundo (todas, incluidas las fallidas).
    pub fn rps(&self) -> f64 {
        let s = self.duracion.as_secs_f64();
        if s == 0.0 { 0.0 } else { self.total as f64 / s }
    }

    pub fn promedio(&self) -> Option<Duration> {
        if self.latencias.is_empty() {
            return None;
        }
        Some(self.latencias.iter().sum::<Duration>() / self.latencias.len() as u32)
    }

    /// Texto final para la terminal.
    pub fn texto(&self) -> String {
        let ms = |d: Option<Duration>| d.map(|d| format!("{:.2} ms", d.as_secs_f64() * 1000.0)).unwrap_or("-".into());
        let mut s = format!(
            "Requests: {} ({} errores) en {:.2}s → {:.1} req/s, {} bytes\nLatencia: prom {} · p50 {} · p90 {} · p99 {} · máx {}\nCódigos:",
            self.total,
            self.errores,
            self.duracion.as_secs_f64(),
            self.rps(),
            self.bytes,
            ms(self.promedio()),
            ms(self.percentil(50.0)),
            ms(self.percentil(90.0)),
            ms(self.percentil(99.0)),
            ms(self.latencias.last().copied())
        );
        for (c, n) in &self.codigos {
            s.push_str(&format!(" [{c}]={n}"));
        }
        for (e, n) in &self.muestras_error {
            s.push_str(&format!("\nError ×{n}: {e}"));
        }
        s
    }
}

/// Ejecuta el benchmark. Cada worker toma "tickets" de un contador atómico
/// compartido hasta agotarlo, así el total es exacto sin coordinación extra.
pub async fn correr(cfg: Config) -> Informe {
    let restantes = Arc::new(AtomicI64::new(cfg.total as i64));
    let inicio = Instant::now();
    let mut tareas = Vec::with_capacity(cfg.concurrencia);
    for _ in 0..cfg.concurrencia.max(1) {
        let restantes = restantes.clone();
        let cfg = cfg.clone();
        tareas.push(tokio::spawn(async move {
            let mut res = Vec::new();
            let mut con: Option<Conexion> = None;
            // fetch_sub devuelve el valor previo: si era <= 0, no quedan tickets.
            while restantes.fetch_sub(1, Ordering::Relaxed) > 0 {
                let t0 = Instant::now();
                let r = tokio::time::timeout(cfg.timeout, async {
                    if con.is_none() {
                        con = Some(Conexion::abrir(&cfg.url).await?);
                    }
                    con.as_mut().expect("recién abierta").get().await
                })
                .await;
                match r {
                    Ok(Ok((codigo, bytes))) => res.push(Resultado::Ok { codigo, latencia: t0.elapsed(), bytes }),
                    Ok(Err(e)) => {
                        con = None; // conexión rota: se reabre en la siguiente
                        res.push(Resultado::Error(e.to_string()));
                    }
                    Err(_) => {
                        con = None;
                        res.push(Resultado::Error("timeout".into()));
                    }
                }
            }
            res
        }));
    }
    let mut todos = Vec::new();
    for t in tareas {
        todos.extend(t.await.unwrap_or_default());
    }
    Informe::desde(todos, inicio.elapsed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn parsea_urls() {
        assert_eq!(Url::parsear("http://localhost:8080/a?b=1").unwrap(), Url { host: "localhost".into(), puerto: 8080, ruta: "/a?b=1".into() });
        assert_eq!(Url::parsear("http://ejemplo.com").unwrap().puerto, 80);
        assert!(Url::parsear("https://x").is_err());
        assert!(Url::parsear("ftp://x").is_err());
        assert!(Url::parsear("http://x:abc").is_err());
    }

    #[test]
    fn percentiles() {
        let r = (1..=100).map(|i| Resultado::Ok { codigo: 200, latencia: Duration::from_millis(i), bytes: 1 }).collect();
        let i = Informe::desde(r, Duration::from_secs(2));
        assert_eq!(i.percentil(50.0), Some(Duration::from_millis(50)));
        assert_eq!(i.percentil(99.0), Some(Duration::from_millis(99)));
        assert_eq!(i.rps(), 50.0);
        assert!(i.texto().contains("[200]=100"));
    }

    /// Servidor HTTP mínimo de prueba: responde keep-alive, alternando
    /// Content-Length y chunked para ejercitar ambos caminos del cliente.
    async fn servidor() -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let puerto = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let (s, _) = l.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut r = BufReader::new(s);
                    let mut n = 0u32;
                    loop {
                        let mut linea = String::new();
                        // Leemos la request hasta la línea vacía.
                        loop {
                            linea.clear();
                            if r.read_line(&mut linea).await.unwrap_or(0) == 0 {
                                return;
                            }
                            if linea == "\r\n" {
                                break;
                            }
                        }
                        n += 1;
                        let resp = if n % 2 == 0 {
                            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhola!\r\n0\r\n\r\n".to_string()
                        } else {
                            "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhola!".to_string()
                        };
                        if r.get_mut().write_all(resp.as_bytes()).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        puerto
    }

    #[tokio::test]
    async fn corrida_completa_contra_servidor_local() {
        let p = servidor().await;
        let cfg = Config { url: Url::parsear(&format!("http://127.0.0.1:{p}/")).unwrap(), total: 200, concurrencia: 8, timeout: Duration::from_secs(5) };
        let inf = correr(cfg).await;
        assert_eq!(inf.total, 200);
        assert_eq!(inf.errores, 0);
        assert_eq!(inf.codigos[&200], 200);
        assert_eq!(inf.bytes, 1000);
    }

    #[tokio::test]
    async fn errores_de_conexion_se_cuentan() {
        // Puerto cerrado: tomamos uno libre y soltamos el listener.
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        let cfg = Config { url: Url::parsear(&format!("http://127.0.0.1:{p}/")).unwrap(), total: 5, concurrencia: 2, timeout: Duration::from_secs(2) };
        let inf = correr(cfg).await;
        assert_eq!(inf.errores, 5);
    }
}
