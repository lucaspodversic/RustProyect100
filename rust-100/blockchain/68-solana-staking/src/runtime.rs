//! Mini-runtime que imita el modelo de cuentas de Solana.
//!
//! En Solana los programas no tienen estado propio: todo vive en **cuentas**
//! que la transacción pasa explícitamente. Cada cuenta tiene un dueño
//! (`owner`, el programa que puede modificar sus datos), lamports y bytes de
//! datos. Las **PDAs** (Program Derived Addresses) son direcciones derivadas de
//! semillas + id del programa que no tienen clave privada: solo el programa
//! puede "firmar" por ellas.

use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Clave pública de 32 bytes (como `solana_program::pubkey::Pubkey`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Pubkey(pub [u8; 32]);

impl Pubkey {
    /// Genera una clave determinista a partir de un nombre (útil en tests).
    pub fn desde_nombre(nombre: &str) -> Self {
        Pubkey(Sha256::digest(nombre.as_bytes()).into())
    }

    /// Deriva una PDA: `sha256(semillas || bump || program_id || "PDA")`.
    ///
    /// En Solana real se busca el `bump` que deje la dirección *fuera* de la
    /// curva ed25519; aquí simulamos esa búsqueda exigiendo que el primer byte
    /// sea par (criterio arbitrario pero determinista) para conservar la idea
    /// del bump canónico.
    pub fn encontrar_pda(semillas: &[&[u8]], programa: &Pubkey) -> (Pubkey, u8) {
        for bump in (0..=255u8).rev() {
            let mut h = Sha256::new();
            for s in semillas {
                h.update(s);
            }
            h.update([bump]);
            h.update(programa.0);
            h.update(b"ProgramDerivedAddress");
            let d: [u8; 32] = h.finalize().into();
            if d[0] % 2 == 0 {
                return (Pubkey(d), bump);
            }
        }
        unreachable!("estadísticamente imposible no encontrar bump")
    }

    /// Representación corta en hex para logs.
    pub fn corta(&self) -> String {
        self.0[..4].iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// Una cuenta en el "ledger".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cuenta {
    pub lamports: u64,
    pub datos: Vec<u8>,
    /// Programa dueño: el único que puede escribir `datos` o restar lamports.
    pub owner: Pubkey,
}

/// Programa del sistema (dueño de las cuentas de usuario).
pub const SYSTEM_PROGRAM: Pubkey = Pubkey([0u8; 32]);

/// Referencia a cuenta dentro de una instrucción (como `AccountMeta`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetaCuenta {
    pub clave: Pubkey,
    pub firmante: bool,
    pub escribible: bool,
}

impl MetaCuenta {
    pub fn firmante(clave: Pubkey) -> Self {
        Self { clave, firmante: true, escribible: true }
    }
    pub fn escribible(clave: Pubkey) -> Self {
        Self { clave, firmante: false, escribible: true }
    }
    pub fn lectura(clave: Pubkey) -> Self {
        Self { clave, firmante: false, escribible: false }
    }
}

/// Ledger en memoria: mapa clave → cuenta + reloj (unix timestamp simulado).
#[derive(Debug, Default, Clone)]
pub struct Banco {
    pub cuentas: HashMap<Pubkey, Cuenta>,
    /// Equivalente al sysvar `Clock::unix_timestamp`.
    pub reloj: i64,
}

impl Banco {
    /// Crea (o recarga) una cuenta de usuario con lamports ("airdrop").
    pub fn airdrop(&mut self, clave: Pubkey, lamports: u64) {
        self.cuentas.entry(clave).or_insert_with(|| Cuenta { owner: SYSTEM_PROGRAM, ..Default::default() }).lamports += lamports;
    }

    pub fn lamports(&self, clave: &Pubkey) -> u64 {
        self.cuentas.get(clave).map(|c| c.lamports).unwrap_or(0)
    }

    /// Suma total de lamports: el runtime real exige que se conserve en cada tx.
    pub fn total_lamports(&self) -> u128 {
        self.cuentas.values().map(|c| c.lamports as u128).sum()
    }
}
