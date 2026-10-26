//! El programa de staking propiamente dicho.
//!
//! Instrucciones (equivalentes a los handlers `#[program]` de Anchor):
//!
//! | Instr. | Cuentas | Efecto |
//! |---|---|---|
//! | `InicializarPool` | autoridad(firma), pool(PDA), tesoro(PDA) | crea el pool |
//! | `Depositar{monto}` | usuario(firma), pool, posición(PDA), bóveda(PDA) | mueve lamports a la bóveda |
//! | `Reclamar` | usuario(firma), pool, posición, tesoro | paga recompensas desde el tesoro |
//! | `Retirar{monto}` | usuario(firma), pool, posición, bóveda | devuelve capital tras el bloqueo |
//! | `Fondear{monto}` | quien sea(firma), tesoro | agrega lamports de recompensa |
//! | `Pausar{pausado}` | autoridad(firma), pool | pausa/reanuda depósitos |
//!
//! Cada handler valida explícitamente firmantes, dueños y PDAs: son los
//! chequeos que Anchor genera con `#[account(...)]` y que, olvidados, causan
//! las vulnerabilidades clásicas de Solana (missing signer, account
//! substitution, type cosplay).

use crate::estado::{Pool, Posicion, ESCALA};
use crate::runtime::{Banco, Cuenta, MetaCuenta, Pubkey};
use thiserror::Error;

/// Id del programa (en Solana sería la dirección donde se desplegó).
pub fn id_programa() -> Pubkey {
    Pubkey::desde_nombre("programa-staking-68")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruccion {
    InicializarPool { tasa_por_segundo: u64, bloqueo_segundos: i64 },
    Depositar { monto: u64 },
    Reclamar,
    Retirar { monto: u64 },
    Fondear { monto: u64 },
    Pausar { pausado: bool },
}

impl Instruccion {
    /// Serializa: 1 byte de tag + argumentos LE (como un enum Borsh).
    pub fn empaquetar(&self) -> Vec<u8> {
        let mut v = Vec::new();
        match self {
            Self::InicializarPool { tasa_por_segundo, bloqueo_segundos } => {
                v.push(0);
                v.extend(tasa_por_segundo.to_le_bytes());
                v.extend(bloqueo_segundos.to_le_bytes());
            }
            Self::Depositar { monto } => {
                v.push(1);
                v.extend(monto.to_le_bytes());
            }
            Self::Reclamar => v.push(2),
            Self::Retirar { monto } => {
                v.push(3);
                v.extend(monto.to_le_bytes());
            }
            Self::Fondear { monto } => {
                v.push(4);
                v.extend(monto.to_le_bytes());
            }
            Self::Pausar { pausado } => {
                v.push(5);
                v.push(*pausado as u8);
            }
        }
        v
    }

    pub fn desempaquetar(d: &[u8]) -> Result<Self, ErrorPrograma> {
        let mut l = crate::estado::Lector::new(d);
        let e = ErrorPrograma::DatosInvalidos;
        let tag = l.u8().ok_or(e.clone())?;
        Ok(match tag {
            0 => Self::InicializarPool { tasa_por_segundo: l.u64().ok_or(e.clone())?, bloqueo_segundos: l.i64().ok_or(e)? },
            1 => Self::Depositar { monto: l.u64().ok_or(e)? },
            2 => Self::Reclamar,
            3 => Self::Retirar { monto: l.u64().ok_or(e)? },
            4 => Self::Fondear { monto: l.u64().ok_or(e)? },
            5 => Self::Pausar { pausado: l.u8().ok_or(e)? != 0 },
            _ => return Err(ErrorPrograma::DatosInvalidos),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErrorPrograma {
    #[error("datos de instrucción inválidos")]
    DatosInvalidos,
    #[error("faltan cuentas")]
    FaltanCuentas,
    #[error("falta la firma requerida")]
    FaltaFirma,
    #[error("la cuenta no es escribible")]
    NoEscribible,
    #[error("PDA inesperada")]
    PdaIncorrecta,
    #[error("dueño de cuenta incorrecto")]
    DuenoIncorrecto,
    #[error("la cuenta ya existe")]
    YaInicializada,
    #[error("la cuenta no está inicializada o es de otro tipo")]
    NoInicializada,
    #[error("fondos insuficientes")]
    FondosInsuficientes,
    #[error("monto cero")]
    MontoCero,
    #[error("pool pausado")]
    Pausado,
    #[error("período de bloqueo vigente")]
    Bloqueado,
    #[error("no autorizado")]
    NoAutorizado,
    #[error("desbordamiento aritmético")]
    Desbordamiento,
}

/// Direcciones PDA usadas por el programa.
pub fn pda_pool(autoridad: &Pubkey) -> (Pubkey, u8) {
    Pubkey::encontrar_pda(&[b"pool", &autoridad.0], &id_programa())
}
pub fn pda_tesoro(pool: &Pubkey) -> (Pubkey, u8) {
    Pubkey::encontrar_pda(&[b"tesoro", &pool.0], &id_programa())
}
pub fn pda_boveda(pool: &Pubkey) -> (Pubkey, u8) {
    Pubkey::encontrar_pda(&[b"boveda", &pool.0], &id_programa())
}
pub fn pda_posicion(pool: &Pubkey, usuario: &Pubkey) -> (Pubkey, u8) {
    Pubkey::encontrar_pda(&[b"stake", &pool.0, &usuario.0], &id_programa())
}

/// Punto de entrada: equivale a `process_instruction` + ejecución atómica.
///
/// Trabajamos sobre una **copia** del banco y solo la confirmamos si todo salió
/// bien: así una instrucción fallida no deja cambios a medias (atomicidad).
pub fn procesar(banco: &mut Banco, metas: &[MetaCuenta], datos: &[u8]) -> Result<(), ErrorPrograma> {
    let mut copia = banco.clone();
    let antes = copia.total_lamports();
    let instr = Instruccion::desempaquetar(datos)?;
    match instr {
        Instruccion::InicializarPool { tasa_por_segundo, bloqueo_segundos } => inicializar(&mut copia, metas, tasa_por_segundo, bloqueo_segundos)?,
        Instruccion::Depositar { monto } => depositar(&mut copia, metas, monto)?,
        Instruccion::Reclamar => reclamar(&mut copia, metas)?,
        Instruccion::Retirar { monto } => retirar(&mut copia, metas, monto)?,
        Instruccion::Fondear { monto } => fondear(&mut copia, metas, monto)?,
        Instruccion::Pausar { pausado } => pausar(&mut copia, metas, pausado)?,
    }
    // Invariante del runtime: los lamports no se crean ni se destruyen.
    assert_eq!(antes, copia.total_lamports(), "violación de conservación de lamports");
    *banco = copia;
    Ok(())
}

// ---------- helpers de validación (lo que Anchor genera con macros) ----------

fn cuenta<'a>(metas: &'a [MetaCuenta], i: usize) -> Result<&'a MetaCuenta, ErrorPrograma> {
    metas.get(i).ok_or(ErrorPrograma::FaltanCuentas)
}

fn exigir_firma(m: &MetaCuenta) -> Result<(), ErrorPrograma> {
    if m.firmante { Ok(()) } else { Err(ErrorPrograma::FaltaFirma) }
}

fn exigir_escritura(m: &MetaCuenta) -> Result<(), ErrorPrograma> {
    if m.escribible { Ok(()) } else { Err(ErrorPrograma::NoEscribible) }
}

fn exigir_pda(m: &MetaCuenta, esperada: Pubkey) -> Result<(), ErrorPrograma> {
    if m.clave == esperada { Ok(()) } else { Err(ErrorPrograma::PdaIncorrecta) }
}

/// Lee un Pool exigiendo que la cuenta pertenezca a este programa.
fn leer_pool(b: &Banco, k: &Pubkey) -> Result<Pool, ErrorPrograma> {
    let c = b.cuentas.get(k).ok_or(ErrorPrograma::NoInicializada)?;
    if c.owner != id_programa() {
        return Err(ErrorPrograma::DuenoIncorrecto);
    }
    Pool::deserializar(&c.datos).ok_or(ErrorPrograma::NoInicializada)
}

fn guardar(b: &mut Banco, k: Pubkey, datos: Vec<u8>) {
    let c = b.cuentas.entry(k).or_insert_with(|| Cuenta { owner: id_programa(), ..Default::default() });
    c.owner = id_programa();
    c.datos = datos;
}

/// Mueve lamports entre cuentas. Solo puede restar de cuentas cuyo dueño es el
/// programa (o de usuarios que firmaron, validado por el llamador).
fn transferir(b: &mut Banco, de: &Pubkey, a: &Pubkey, monto: u64) -> Result<(), ErrorPrograma> {
    let origen = b.cuentas.get_mut(de).ok_or(ErrorPrograma::FondosInsuficientes)?;
    origen.lamports = origen.lamports.checked_sub(monto).ok_or(ErrorPrograma::FondosInsuficientes)?;
    let destino = b.cuentas.entry(*a).or_insert_with(|| Cuenta { owner: id_programa(), ..Default::default() });
    destino.lamports = destino.lamports.checked_add(monto).ok_or(ErrorPrograma::Desbordamiento)?;
    Ok(())
}

// ------------------------------- handlers -----------------------------------

fn inicializar(b: &mut Banco, m: &[MetaCuenta], tasa: u64, bloqueo: i64) -> Result<(), ErrorPrograma> {
    let autoridad = cuenta(m, 0)?;
    let pool = cuenta(m, 1)?;
    exigir_firma(autoridad)?;
    exigir_escritura(pool)?;
    let (pda, bump) = pda_pool(&autoridad.clave);
    exigir_pda(pool, pda)?;
    if b.cuentas.get(&pda).is_some_and(|c| !c.datos.is_empty()) {
        return Err(ErrorPrograma::YaInicializada);
    }
    let p = Pool { autoridad: autoridad.clave, tasa_por_segundo: tasa, bloqueo_segundos: bloqueo.max(0), total_stakeado: 0, pausado: false, bump };
    guardar(b, pda, p.serializar());
    Ok(())
}

fn depositar(b: &mut Banco, m: &[MetaCuenta], monto: u64) -> Result<(), ErrorPrograma> {
    let (usuario, pool_m, pos_m, boveda_m) = (cuenta(m, 0)?, cuenta(m, 1)?, cuenta(m, 2)?, cuenta(m, 3)?);
    exigir_firma(usuario)?;
    exigir_escritura(pos_m)?;
    exigir_escritura(boveda_m)?;
    if monto == 0 {
        return Err(ErrorPrograma::MontoCero);
    }
    let mut pool = leer_pool(b, &pool_m.clave)?;
    if pool.pausado {
        return Err(ErrorPrograma::Pausado);
    }
    let (pda_pos, bump) = pda_posicion(&pool_m.clave, &usuario.clave);
    exigir_pda(pos_m, pda_pos)?;
    exigir_pda(boveda_m, pda_boveda(&pool_m.clave).0)?;
    let ahora = b.reloj;
    let mut pos = match b.cuentas.get(&pda_pos).and_then(|c| Posicion::deserializar(&c.datos)) {
        Some(p) => p,
        None => Posicion { dueno: usuario.clave, pool: pool_m.clave, monto: 0, ultimo_calculo: ahora, desde: ahora, pendiente: 0, bump },
    };
    pos.devengar(pool.tasa_por_segundo, ahora);
    // El usuario es dueño "system": el runtime permite restarle porque firmó.
    transferir(b, &usuario.clave, &boveda_m.clave, monto)?;
    pos.monto = pos.monto.checked_add(monto).ok_or(ErrorPrograma::Desbordamiento)?;
    if pos.monto == monto {
        pos.desde = ahora; // reinicia el bloqueo si la posición estaba vacía
    }
    pool.total_stakeado = pool.total_stakeado.checked_add(monto).ok_or(ErrorPrograma::Desbordamiento)?;
    guardar(b, pda_pos, pos.serializar());
    guardar(b, pool_m.clave, pool.serializar());
    Ok(())
}

fn leer_posicion_propia(b: &Banco, usuario: &MetaCuenta, pool_k: &Pubkey, pos_m: &MetaCuenta) -> Result<Posicion, ErrorPrograma> {
    exigir_pda(pos_m, pda_posicion(pool_k, &usuario.clave).0)?;
    let c = b.cuentas.get(&pos_m.clave).ok_or(ErrorPrograma::NoInicializada)?;
    if c.owner != crate::programa::id_programa() {
        return Err(ErrorPrograma::DuenoIncorrecto);
    }
    let pos = Posicion::deserializar(&c.datos).ok_or(ErrorPrograma::NoInicializada)?;
    // `has_one = dueno` en Anchor.
    if pos.dueno != usuario.clave || pos.pool != *pool_k {
        return Err(ErrorPrograma::NoAutorizado);
    }
    Ok(pos)
}

fn reclamar(b: &mut Banco, m: &[MetaCuenta]) -> Result<(), ErrorPrograma> {
    let (usuario, pool_m, pos_m, tesoro_m) = (cuenta(m, 0)?, cuenta(m, 1)?, cuenta(m, 2)?, cuenta(m, 3)?);
    exigir_firma(usuario)?;
    let pool = leer_pool(b, &pool_m.clave)?;
    exigir_pda(tesoro_m, pda_tesoro(&pool_m.clave).0)?;
    let mut pos = leer_posicion_propia(b, usuario, &pool_m.clave, pos_m)?;
    pos.devengar(pool.tasa_por_segundo, b.reloj);
    let a_pagar = (pos.pendiente / ESCALA) as u64; // parte entera
    let disponible = b.lamports(&tesoro_m.clave);
    let pago = a_pagar.min(disponible); // si el tesoro no alcanza, paga lo que hay
    transferir(b, &tesoro_m.clave, &usuario.clave, pago)?;
    pos.pendiente -= pago as u128 * ESCALA;
    guardar(b, pos_m.clave, pos.serializar());
    Ok(())
}

fn retirar(b: &mut Banco, m: &[MetaCuenta], monto: u64) -> Result<(), ErrorPrograma> {
    let (usuario, pool_m, pos_m, boveda_m) = (cuenta(m, 0)?, cuenta(m, 1)?, cuenta(m, 2)?, cuenta(m, 3)?);
    exigir_firma(usuario)?;
    if monto == 0 {
        return Err(ErrorPrograma::MontoCero);
    }
    let mut pool = leer_pool(b, &pool_m.clave)?;
    exigir_pda(boveda_m, pda_boveda(&pool_m.clave).0)?;
    let mut pos = leer_posicion_propia(b, usuario, &pool_m.clave, pos_m)?;
    if b.reloj < pos.desde + pool.bloqueo_segundos {
        return Err(ErrorPrograma::Bloqueado);
    }
    if monto > pos.monto {
        return Err(ErrorPrograma::FondosInsuficientes);
    }
    pos.devengar(pool.tasa_por_segundo, b.reloj); // las recompensas quedan pendientes
    transferir(b, &boveda_m.clave, &usuario.clave, monto)?;
    pos.monto -= monto;
    pool.total_stakeado -= monto;
    guardar(b, pos_m.clave, pos.serializar());
    guardar(b, pool_m.clave, pool.serializar());
    Ok(())
}

fn fondear(b: &mut Banco, m: &[MetaCuenta], monto: u64) -> Result<(), ErrorPrograma> {
    let (quien, pool_m, tesoro_m) = (cuenta(m, 0)?, cuenta(m, 1)?, cuenta(m, 2)?);
    exigir_firma(quien)?;
    leer_pool(b, &pool_m.clave)?;
    exigir_pda(tesoro_m, pda_tesoro(&pool_m.clave).0)?;
    transferir(b, &quien.clave, &tesoro_m.clave, monto)
}

fn pausar(b: &mut Banco, m: &[MetaCuenta], pausado: bool) -> Result<(), ErrorPrograma> {
    let (aut, pool_m) = (cuenta(m, 0)?, cuenta(m, 1)?);
    exigir_firma(aut)?;
    let mut pool = leer_pool(b, &pool_m.clave)?;
    if pool.autoridad != aut.clave {
        return Err(ErrorPrograma::NoAutorizado);
    }
    pool.pausado = pausado;
    guardar(b, pool_m.clave, pool.serializar());
    Ok(())
}

/// Lee una posición (para clientes/tests).
pub fn ver_posicion(b: &Banco, pool: &Pubkey, usuario: &Pubkey) -> Option<Posicion> {
    b.cuentas.get(&pda_posicion(pool, usuario).0).and_then(|c| Posicion::deserializar(&c.datos))
}

/// Lee el pool (para clientes/tests).
pub fn ver_pool(b: &Banco, pool: &Pubkey) -> Option<Pool> {
    b.cuentas.get(pool).and_then(|c| Pool::deserializar(&c.datos))
}
