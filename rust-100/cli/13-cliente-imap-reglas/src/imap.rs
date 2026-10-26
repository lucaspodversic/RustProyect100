//! Cliente IMAP4rev1 mínimo (RFC 3501), genérico sobre cualquier `Read + Write`.
//!
//! Ser genérico permite usarlo con `TcpStream` (texto plano), con un stream TLS
//! de otra librería, o con un servidor simulado en los tests.
//!
//! ## Protocolo en dos líneas
//! El cliente manda `TAG COMANDO args\r\n`; el servidor responde con 0..n líneas
//! "no etiquetadas" (`* ...`) y termina con `TAG OK|NO|BAD texto`. Las respuestas
//! pueden incluir **literales** `{n}\r\n<n bytes>` (así viajan las cabeceras).

use crate::reglas::Mensaje;
use std::io::{self, BufRead, BufReader, Read, Write};

/// Línea no etiquetada, con el literal (si lo hubo) separado.
#[derive(Debug, Clone)]
pub struct Respuesta {
    pub texto: String,
    pub literal: Option<Vec<u8>>,
}

/// Error de IMAP: E/S o respuesta NO/BAD del servidor.
#[derive(Debug)]
pub enum ErrorImap {
    Io(io::Error),
    Servidor(String),
    Protocolo(String),
}
impl std::fmt::Display for ErrorImap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorImap::Io(e) => write!(f, "E/S: {e}"),
            ErrorImap::Servidor(s) => write!(f, "el servidor respondió: {s}"),
            ErrorImap::Protocolo(s) => write!(f, "protocolo: {s}"),
        }
    }
}
impl std::error::Error for ErrorImap {}
impl From<io::Error> for ErrorImap {
    fn from(e: io::Error) -> Self {
        ErrorImap::Io(e)
    }
}

/// Entrecomilla un string IMAP (escapa `\` y `"`).
pub fn comillas(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Cliente sobre un stream bidireccional.
pub struct Cliente<S: Read + Write> {
    lector: BufReader<S>,
    contador: u32,
}

impl<S: Read + Write> Cliente<S> {
    /// Envuelve el stream y consume el saludo `* OK ...` del servidor.
    pub fn nuevo(stream: S) -> Result<Self, ErrorImap> {
        let mut c = Cliente { lector: BufReader::new(stream), contador: 0 };
        let saludo = c.leer_linea()?;
        if !saludo.starts_with("* OK") && !saludo.starts_with("* PREAUTH") {
            return Err(ErrorImap::Protocolo(format!("saludo inesperado: {saludo}")));
        }
        Ok(c)
    }

    fn leer_linea(&mut self) -> Result<String, ErrorImap> {
        let mut l = String::new();
        if self.lector.read_line(&mut l)? == 0 {
            return Err(ErrorImap::Protocolo("conexión cerrada".into()));
        }
        Ok(l.trim_end_matches(['\r', '\n']).to_string())
    }

    /// Envía un comando y junta las respuestas hasta la línea etiquetada.
    pub fn comando(&mut self, cmd: &str) -> Result<Vec<Respuesta>, ErrorImap> {
        self.contador += 1;
        let tag = format!("A{:03}", self.contador);
        let s = self.lector.get_mut();
        write!(s, "{tag} {cmd}\r\n")?;
        s.flush()?;
        let mut resp = Vec::new();
        loop {
            let mut linea = self.leer_linea()?;
            if let Some(resto) = linea.strip_prefix(&format!("{tag} ")) {
                return if resto.starts_with("OK") {
                    Ok(resp)
                } else {
                    Err(ErrorImap::Servidor(resto.to_string()))
                };
            }
            let mut literal = None;
            // ¿La línea termina en {n}? Entonces vienen n bytes crudos y luego el resto.
            if linea.ends_with('}') {
                if let Some(ini) = linea.rfind('{') {
                    let n: usize = linea[ini + 1..linea.len() - 1]
                        .parse()
                        .map_err(|_| ErrorImap::Protocolo(format!("literal inválido: {linea}")))?;
                    let mut buf = vec![0u8; n];
                    self.lector.read_exact(&mut buf)?;
                    literal = Some(buf);
                    linea.truncate(ini);
                    let cola = self.leer_linea()?; // típicamente ")"
                    linea.push_str(&cola);
                }
            }
            resp.push(Respuesta { texto: linea, literal });
        }
    }

    pub fn login(&mut self, usuario: &str, clave: &str) -> Result<(), ErrorImap> {
        self.comando(&format!("LOGIN {} {}", comillas(usuario), comillas(clave))).map(|_| ())
    }

    /// Selecciona una carpeta; devuelve la cantidad de mensajes (respuesta `* n EXISTS`).
    pub fn seleccionar(&mut self, carpeta: &str) -> Result<u32, ErrorImap> {
        let r = self.comando(&format!("SELECT {}", comillas(carpeta)))?;
        Ok(r.iter()
            .find_map(|x| x.texto.strip_prefix("* ")?.strip_suffix(" EXISTS")?.parse().ok())
            .unwrap_or(0))
    }

    /// UIDs que cumplen un criterio de búsqueda (`ALL`, `UNSEEN`, ...).
    pub fn buscar(&mut self, criterio: &str) -> Result<Vec<u32>, ErrorImap> {
        let r = self.comando(&format!("UID SEARCH {criterio}"))?;
        Ok(r.iter()
            .filter_map(|x| x.texto.strip_prefix("* SEARCH"))
            .flat_map(|s| s.split_whitespace().filter_map(|n| n.parse().ok()).collect::<Vec<u32>>())
            .collect())
    }

    /// Trae From y Subject de los UIDs dados (sin marcarlos como leídos: BODY.PEEK).
    pub fn cabeceras(&mut self, uids: &[u32]) -> Result<Vec<Mensaje>, ErrorImap> {
        if uids.is_empty() {
            return Ok(vec![]);
        }
        let set = uids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let r = self.comando(&format!("UID FETCH {set} (UID BODY.PEEK[HEADER.FIELDS (FROM SUBJECT)])"))?;
        let mut v = Vec::new();
        for x in r {
            let Some(lit) = x.literal else { continue };
            // Buscamos "UID n" dentro de la respuesta FETCH.
            let uid = x
                .texto
                .split_whitespace()
                .skip_while(|t| t.trim_start_matches('(') != "UID")
                .nth(1)
                .and_then(|n| n.trim_end_matches(')').parse().ok())
                .ok_or_else(|| ErrorImap::Protocolo(format!("FETCH sin UID: {}", x.texto)))?;
            let (de, asunto) = parsear_cabeceras(&String::from_utf8_lossy(&lit));
            v.push(Mensaje { uid, de, asunto });
        }
        Ok(v)
    }

    pub fn agregar_bandera(&mut self, uid: u32, bandera: &str) -> Result<(), ErrorImap> {
        self.comando(&format!("UID STORE {uid} +FLAGS.SILENT ({bandera})")).map(|_| ())
    }

    /// Mover portable: COPY + \Deleted (el EXPUNGE se hace al final, una sola vez).
    pub fn mover(&mut self, uid: u32, destino: &str) -> Result<(), ErrorImap> {
        self.comando(&format!("UID COPY {uid} {}", comillas(destino)))?;
        self.agregar_bandera(uid, "\\Deleted")
    }

    pub fn expunge(&mut self) -> Result<(), ErrorImap> {
        self.comando("EXPUNGE").map(|_| ())
    }

    pub fn logout(&mut self) -> Result<(), ErrorImap> {
        self.comando("LOGOUT").map(|_| ())
    }
}

/// Extrae From y Subject de un bloque de cabeceras RFC 5322, uniendo líneas
/// "plegadas" (las que empiezan con espacio/tab continúan la anterior).
/// No decodifica MIME encoded-words (`=?UTF-8?B?...?=`): limitación documentada.
pub fn parsear_cabeceras(bloque: &str) -> (String, String) {
    let mut desplegado: Vec<String> = Vec::new();
    for l in bloque.split("\r\n").flat_map(|l| l.split('\n')) {
        if (l.starts_with(' ') || l.starts_with('\t')) && !desplegado.is_empty() {
            let u = desplegado.last_mut().expect("no vacío");
            u.push(' ');
            u.push_str(l.trim());
        } else if !l.is_empty() {
            desplegado.push(l.to_string());
        }
    }
    let buscar = |nombre: &str| {
        desplegado
            .iter()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.eq_ignore_ascii_case(nombre).then(|| v.trim().to_string())
            })
            .unwrap_or_default()
    };
    (buscar("From"), buscar("Subject"))
}
