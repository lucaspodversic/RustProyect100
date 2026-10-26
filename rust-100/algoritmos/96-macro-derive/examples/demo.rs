//! `cargo run --example demo`: muestra el resultado de la macro.
//! Para ver el código expandido: `cargo install cargo-expand && cargo expand --example demo`.
use describir::Describir;

#[derive(Describir, Debug)]
struct Cuenta {
    titular: String,
    saldo_centimos: i64,
    #[describir(omitir)]
    pin: u16,
}

#[derive(Describir)]
enum Evento {
    Ingreso(i64),
    Retiro { cantidad: i64, #[describir(renombrar = "cajero")] atm: u32 },
}

fn main() {
    let c = Cuenta { titular: "Lucía".into(), saldo_centimos: 12_345, pin: 1234 };
    println!("Debug:     {c:?}");
    println!("Describir: {}", c.describir());
    println!("Campos de {}: {:?}", Cuenta::nombre_tipo(), Cuenta::campos());
    println!("¿PIN correcto?: {}", c.pin == 1234); // el campo existe; solo se oculta al describir
    for e in [Evento::Ingreso(500), Evento::Retiro { cantidad: 200, atm: 77 }] {
        println!("  {}", e.describir());
    }
}
