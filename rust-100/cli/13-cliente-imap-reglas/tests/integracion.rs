//! Tests de integración: cliente real ↔ servidor IMAP simulado por TCP local.

use cliente_imap_reglas::imap::{parsear_cabeceras, Cliente, ErrorImap};
use cliente_imap_reglas::reglas::{self, Accion, Mensaje};
use cliente_imap_reglas::{procesar, servidor_simulado as srv};
use std::net::TcpStream;

fn reglas_ejemplo() -> Vec<reglas::Regla> {
    reglas::cargar(include_str!("../reglas.ejemplo.json")).unwrap()
}

fn conectar(b: &srv::Buzon) -> Cliente<TcpStream> {
    let dir = srv::iniciar(b.clone(), "yo@x", "cl\"ave");
    let mut c = Cliente::nuevo(TcpStream::connect(dir).unwrap()).unwrap();
    c.login("yo@x", "cl\"ave").unwrap(); // comillas escapadas correctamente
    c
}

#[test]
fn login_incorrecto_da_error_de_servidor() {
    let dir = srv::iniciar(srv::buzon_demo(), "u", "k");
    let mut c = Cliente::nuevo(TcpStream::connect(dir).unwrap()).unwrap();
    assert!(matches!(c.login("u", "mal"), Err(ErrorImap::Servidor(_))));
}

#[test]
fn lee_cabeceras_por_literal() {
    let b = srv::buzon_demo();
    let mut c = conectar(&b);
    assert_eq!(c.seleccionar("INBOX").unwrap(), 4);
    let uids = c.buscar("ALL").unwrap();
    assert_eq!(uids, vec![1, 2, 3, 4]);
    let m = c.cabeceras(&uids).unwrap();
    assert_eq!(m[2].asunto, "Asado el sábado?"); // UTF-8 en literal: largo en bytes
    assert!(m[0].de.contains("facturacion@luz.example"));
}

#[test]
fn simulacion_no_modifica() {
    let b = srv::buzon_demo();
    let mut c = conectar(&b);
    c.seleccionar("INBOX").unwrap();
    let h = procesar(&mut c, &reglas_ejemplo(), "ALL", true).unwrap();
    assert_eq!(h.len(), 4);
    assert_eq!(b.lock().unwrap()["INBOX"].len(), 4);
    assert!(b.lock().unwrap()["INBOX"].iter().all(|m| m.banderas.is_empty()));
}

#[test]
fn aplica_reglas_de_verdad() {
    let b = srv::buzon_demo();
    let mut c = conectar(&b);
    c.seleccionar("INBOX").unwrap();
    let h = procesar(&mut c, &reglas_ejemplo(), "ALL", false).unwrap();
    assert_eq!(h[0].accion, Accion::Mover("Facturas".into()));
    let g = b.lock().unwrap();
    // Factura movida y spam borrado → quedan 2 en INBOX.
    let inbox: Vec<u32> = g["INBOX"].iter().map(|m| m.uid).collect();
    assert_eq!(inbox, vec![2, 3]);
    assert_eq!(g["Facturas"].len(), 1);
    assert!(g["INBOX"][0].banderas.contains(&"\\Seen".to_string()));
    assert!(g["INBOX"][1].banderas.contains(&"\\Flagged".to_string()));
    drop(g);
    // Segunda pasada con UNSEEN: la promo ya está leída, no se vuelve a procesar.
    let h2 = procesar(&mut c, &reglas_ejemplo(), "UNSEEN", false).unwrap();
    assert_eq!(h2.len(), 1); // solo "amigos" (destacado, no leído)
}

#[test]
fn mover_a_carpeta_inexistente_falla() {
    let b = srv::buzon_demo();
    let mut c = conectar(&b);
    c.seleccionar("INBOX").unwrap();
    assert!(c.mover(1, "NoExiste").is_err());
}

#[test]
fn motor_de_reglas_puro() {
    let r = reglas_ejemplo();
    let m = |de: &str, a: &str| Mensaje { uid: 1, de: de.into(), asunto: a.into() };
    assert_eq!(reglas::evaluar(&r, &m("x@spam.example", "Ganaste!")).unwrap().nombre, "spam");
    // Spam exige ambas condiciones (AND).
    assert!(reglas::evaluar(&r, &m("x@spam.example", "hola")).is_none());
    // Regla sin condiciones nunca aplica.
    let vacia: Vec<reglas::Regla> = reglas::cargar(r#"[{"nombre":"todo","accion":"borrar"}]"#).unwrap();
    assert!(reglas::evaluar(&vacia, &m("a", "b")).is_none());
}

#[test]
fn cabeceras_plegadas() {
    let (de, asunto) = parsear_cabeceras("Subject: muy\r\n largo\r\nFROM: a@b\r\n\r\n");
    assert_eq!((de.as_str(), asunto.as_str()), ("a@b", "muy largo"));
}
