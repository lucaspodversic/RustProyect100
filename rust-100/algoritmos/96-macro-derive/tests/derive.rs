//! Tests de integración: usan la macro "desde fuera", como un usuario real.

use describir::{describir_todos, Describir};

#[derive(Describir)]
struct Vacio;

#[derive(Describir)]
#[describir(nombre = "Cfg")]
struct Configuracion {
    host: &'static str,
    puerto: u16,
    #[describir(omitir)]
    _token: &'static str,
}

#[derive(Describir)]
struct Par<T, U> {
    a: T,
    b: Option<U>,
}

#[derive(Describir)]
struct SoloSecreto {
    #[describir(omitir)]
    _x: u8,
}

#[derive(Describir)]
#[allow(dead_code)] // el campo omitido nunca se lee (justo lo que queremos)
struct TuplaRenombrada(#[describir(renombrar = "id")] u32, #[describir(omitir)] u8, bool);

#[derive(Describir)]
enum Forma {
    Circulo { radio: f32 },
    #[describir(renombrar = "Rect")]
    Rectangulo(u32, u32),
    Nada,
}

#[test]
fn struct_unidad() {
    assert_eq!(Vacio.describir(), "Vacio");
    assert!(Vacio::campos().is_empty());
}

#[test]
fn nombre_de_tipo_y_omitidos() {
    let c = Configuracion { host: "localhost", puerto: 8080, _token: "xyz" };
    assert_eq!(c.describir(), r#"Cfg { host: "localhost", puerto: 8080, .. }"#);
    assert_eq!(Configuracion::nombre_tipo(), "Cfg");
    assert!(!c.describir().contains("xyz"));
}

#[test]
fn todos_omitidos() {
    assert_eq!(SoloSecreto { _x: 1 }.describir(), "SoloSecreto { .. }");
}

#[test]
fn genericos_reciben_bound_debug() {
    let p = Par { a: 1.5, b: Some("hola") };
    assert_eq!(p.describir(), r#"Par { a: 1.5, b: Some("hola") }"#);
}

#[test]
fn tupla_con_renombre_y_omision() {
    assert_eq!(TuplaRenombrada(7, 0, true).describir(), "TuplaRenombrada(id: 7, true, ..)");
    assert_eq!(TuplaRenombrada::campos(), &["id", "2"]);
}

#[test]
fn enums() {
    assert_eq!(Forma::Circulo { radio: 2.0 }.describir(), "Forma::Circulo { radio: 2.0 }");
    assert_eq!(Forma::Rectangulo(3, 4).describir(), "Forma::Rect(3, 4)");
    assert_eq!(Forma::Nada.describir(), "Forma::Nada");
    assert_eq!(Forma::campos(), &["Circulo", "Rectangulo", "Nada"]);
}

#[test]
fn objeto_de_trait() {
    let v: [&dyn Describir; 2] = [&Vacio, &Forma::Nada];
    assert_eq!(describir_todos(&v), "Vacio\nForma::Nada");
}
