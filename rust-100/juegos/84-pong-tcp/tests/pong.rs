use pong_tcp::juego::*;
use pong_tcp::protocolo::*;
use pong_tcp::red::*;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

#[test]
fn paredes_reflejan_y_paletas_limitadas() {
    let mut j = Juego::new(5);
    j.estado.pelota = (40.0, 0.5);
    j.estado.vel = (0.0, -30.0);
    let ev = j.paso([0, 0], 0.05);
    assert!(ev.contains(&Evento::Pared));
    assert!(j.estado.vel.1 > 0.0 && j.estado.pelota.1 >= 0.0);
    for _ in 0..200 {
        j.paso([-1, 1], 0.05);
    }
    assert_eq!(j.estado.paletas[0], ALTO_PALETA / 2.0);
    assert_eq!(j.estado.paletas[1], ALTO - ALTO_PALETA / 2.0);
}

#[test]
fn rebote_en_paleta_con_angulo_segun_impacto_y_acelera() {
    let mut j = Juego::new(5);
    j.estado.paletas[0] = 12.0;
    j.estado.pelota = (3.0, 12.0);
    j.estado.vel = (-30.0, 0.0);
    let ev = j.paso([0, 0], 0.05);
    assert!(ev.contains(&Evento::Rebote(0)));
    assert!(j.estado.vel.0 > 0.0);
    assert!(j.estado.vel.1.abs() < 1.0, "centro: sale recta");
    assert!((j.estado.vel.0 - 31.8).abs() < 0.1, "acelera 6%");
    // Pegar en el borde inferior: sale hacia abajo con ángulo grande.
    j.estado.pelota = (3.0, 14.4);
    j.estado.vel = (-30.0, 0.0);
    j.paso([0, 0], 0.05);
    assert!(j.estado.vel.1 > 15.0);
}

#[test]
fn pelota_rapida_no_atraviesa_la_paleta() {
    let mut j = Juego::new(5);
    j.estado.paletas[1] = 12.0;
    j.estado.pelota = (ANCHO - 3.0, 12.0);
    j.estado.vel = (VEL_MAX, 0.0); // recorre 3.5 unidades en un paso
    let ev = j.paso([0, 0], 0.05);
    assert!(ev.contains(&Evento::Rebote(1)), "{ev:?}");
}

#[test]
fn puntos_saque_y_fin() {
    let mut j = Juego::new(2);
    j.estado.paletas[0] = 2.5; // lejos de la pelota
    j.estado.pelota = (1.0, 20.0);
    j.estado.vel = (-30.0, 0.0);
    let ev = j.paso([0, 0], 0.1);
    assert!(ev.contains(&Evento::Punto(1)));
    assert_eq!(j.estado.puntos, [0, 1]);
    assert_eq!(j.estado.pelota, (ANCHO / 2.0, ALTO / 2.0));
    assert!(j.estado.vel.0 < 0.0, "saca hacia quien perdió");
    j.estado.pelota = (1.0, 20.0);
    j.estado.vel = (-30.0, 0.0);
    let ev = j.paso([0, 0], 0.1);
    assert!(ev.contains(&Evento::Fin(1)));
    assert_eq!(j.estado.ganador, Some(1));
    let antes = j.estado.clone();
    j.paso([1, 1], 0.1);
    assert_eq!(j.estado, antes, "terminado: no cambia");
}

#[test]
fn ia_sigue_la_pelota_y_render() {
    let mut j = Juego::new(5);
    j.estado.vel = (-10.0, 0.0);
    j.estado.pelota = (30.0, 20.0);
    assert_eq!(ia(&j.estado, 0, 0.5), 1);
    assert_eq!(ia(&j.estado, 1, 0.5), 0, "no viene: vuelve al centro (ya está)");
    let r = render(&j.estado, 40, 12);
    assert!(r.contains('●') && r.contains('█'));
    assert_eq!(r.lines().count(), 15);
}

#[test]
fn protocolo_ida_y_vuelta_y_validacion() {
    let j = Juego::new(3);
    let l = MsgServidor::Estado(j.estado.clone()).a_linea();
    match MsgServidor::parsear(&l).unwrap() {
        MsgServidor::Estado(e) => {
            assert_eq!(e.puntos, j.estado.puntos);
            assert!((e.pelota.0 - j.estado.pelota.0).abs() < 1e-3);
        }
        otro => panic!("{otro:?}"),
    }
    for m in [MsgServidor::Bienvenido(1), MsgServidor::Fin(None), MsgServidor::Fin(Some(0)), MsgServidor::Inicio("ana".into())] {
        assert_eq!(MsgServidor::parsear(&m.a_linea()).unwrap(), m);
    }
    assert_eq!(MsgCliente::parsear("IN -1\n"), Ok(MsgCliente::Entrada(-1)));
    assert!(MsgCliente::parsear("IN 5").is_err());
    assert!(MsgCliente::parsear("BORRAR TODO").is_err());
    assert_eq!(MsgCliente::parsear(&"x".repeat(300)), Err(ErrorProtocolo::Larga));
    assert_eq!(MsgCliente::parsear("HOLA <script>ana\n"), Ok(MsgCliente::Hola("scriptana".into())));
    assert!(MsgServidor::parsear("ESTADO 1 2 3").is_err());
    assert!(MsgServidor::parsear("ESTADO 1 NaN 0 0 0 0 0 0 0").is_err());
    assert!(MsgServidor::parsear("BIENVENIDO 7").is_err());
}

fn cfg_rapida(puntos: u32) -> Config {
    Config { para_ganar: puntos, tick_real: Duration::ZERO, max_ticks: 100_000, ..Config::default() }
}

#[test]
fn partida_completa_por_tcp_en_localhost() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let srv = thread::spawn(move || servir(l, cfg_rapida(2)));
    let a = addr.clone();
    let c0 = thread::spawn(move || jugar(&a, "ana", controlador_ia(0.5), |_| {}));
    thread::sleep(Duration::from_millis(50)); // asegura el orden de llegada
    let r1 = jugar(&addr, "beto", controlador_ia(3.0), |_| {}).unwrap();
    let r0 = c0.join().unwrap().unwrap();
    let res = srv.join().unwrap().unwrap();
    assert_eq!(res.nombres, ["ana".to_string(), "beto".to_string()]);
    assert_eq!((r0.jugador, r1.jugador), (0, 1));
    assert_eq!((r0.rival.as_str(), r1.rival.as_str()), ("beto", "ana"));
    assert!(!res.por_abandono);
    assert!(res.ganador.is_some());
    assert_eq!(res.puntos[res.ganador.unwrap()], 2);
    assert_eq!(r0.fin, Some(res.ganador));
    assert!(r0.estados > 10 && r1.estados > 10);
}

#[test]
fn abandono_da_la_victoria_al_otro() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let srv = thread::spawn(move || servir(l, Config { tick_real: Duration::from_millis(2), ..cfg_rapida(50) }));
    let a = addr.clone();
    let c0 = thread::spawn(move || jugar(&a, "fiel", controlador_ia(0.5), |_| {}));
    thread::sleep(Duration::from_millis(50));
    // El segundo saluda, recibe algunos estados y corta.
    let mut s = TcpStream::connect(&addr).unwrap();
    s.write_all(b"HOLA desertor\n").unwrap();
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut l = String::new();
    for _ in 0..5 {
        l.clear();
        r.read_line(&mut l).unwrap();
    }
    s.write_all(b"CHAU\n").unwrap();
    drop(r);
    drop(s);
    let res = srv.join().unwrap().unwrap();
    assert!(res.por_abandono);
    assert_eq!(res.ganador, Some(0));
    assert_eq!(c0.join().unwrap().unwrap().fin, Some(Some(0)));
}

#[test]
fn tercer_cliente_rechazado_y_basura_ignorada() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let srv = thread::spawn(move || servir(l, Config { tick_real: Duration::from_millis(1), max_ticks: 3_000, ..cfg_rapida(99) }));
    let a = addr.clone();
    let c0 = thread::spawn(move || jugar(&a, "uno", controlador_ia(0.5), |_| {}));
    thread::sleep(Duration::from_millis(30));
    // Jugador 2 "ruidoso": manda basura y luego juega normal.
    let a = addr.clone();
    let c1 = thread::spawn(move || {
        let mut s = TcpStream::connect(&a).unwrap();
        s.write_all(b"HOLA dos\nBASURA\nIN 9\nIN 1\n").unwrap();
        let mut r = BufReader::new(s);
        let mut l = String::new();
        let mut fin = false;
        while r.read_line(&mut l).unwrap() > 0 {
            if l.starts_with("FIN") {
                fin = true;
                break;
            }
            l.clear();
        }
        fin
    });
    thread::sleep(Duration::from_millis(100));
    let mut tercero = BufReader::new(TcpStream::connect(&addr).unwrap());
    let mut resp = String::new();
    tercero.read_line(&mut resp).unwrap();
    assert_eq!(resp, "ERROR lleno\n");
    let res = srv.join().unwrap().unwrap();
    assert!(c1.join().unwrap(), "la basura no tumbó la conexión");
    assert!(c0.join().unwrap().is_ok());
    assert!(res.ticks <= 3_000);
}
