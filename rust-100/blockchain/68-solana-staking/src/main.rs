//! Demo: un pool con dos usuarios, recompensas por tiempo y retiro.
use solana_staking::cliente::Cliente;
use solana_staking::programa::ver_posicion;
use solana_staking::runtime::{Banco, Pubkey};

fn main() {
    let mut banco = Banco::default();
    let (aut, ana, beto) = (Pubkey::desde_nombre("autoridad"), Pubkey::desde_nombre("ana"), Pubkey::desde_nombre("beto"));
    for k in [aut, ana, beto] {
        banco.airdrop(k, 1_000_000);
    }
    // Tasa: 0,0001% por segundo (1000/1e9), bloqueo 60 s.
    let mut c = Cliente::inicializar(&mut banco, aut, 1_000, 60).unwrap();
    c.fondear(aut, 500_000).unwrap();
    c.depositar(ana, 100_000).unwrap();
    c.banco.reloj += 30;
    c.depositar(beto, 200_000).unwrap();
    c.banco.reloj += 3_600;
    c.reclamar(ana).unwrap();
    c.retirar(ana, 100_000).unwrap();
    let pool = c.pool;
    println!("Ana: {} lamports (ganó {} de recompensa)", banco.lamports(&ana), banco.lamports(&ana) - 1_000_000);
    println!("Posición de Beto: {:?}", ver_posicion(&banco, &pool, &beto).map(|p| p.monto));
}
