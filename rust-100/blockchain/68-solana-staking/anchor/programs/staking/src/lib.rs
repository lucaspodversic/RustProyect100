// Versión Anchor del mismo programa (NO se compila con `cargo test` de este
// crate: requiere `anchor-cli` + toolchain Solana). Ver README para el path.
//
// Las restricciones `#[account(...)]` corresponden 1:1 a los chequeos manuales
// de `src/programa.rs` (firma, PDA con seeds+bump, has_one, owner implícito).
use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("Stak1ng11111111111111111111111111111111111");

const ESCALA: u128 = 1_000_000_000;

#[program]
pub mod staking {
    use super::*;

    /// Crea el pool (PDA ["pool", autoridad]).
    pub fn inicializar_pool(ctx: Context<InicializarPool>, tasa_por_segundo: u64, bloqueo_segundos: i64) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.autoridad = ctx.accounts.autoridad.key();
        pool.tasa_por_segundo = tasa_por_segundo;
        pool.bloqueo_segundos = bloqueo_segundos.max(0);
        pool.bump = ctx.bumps.pool;
        Ok(())
    }

    /// Deposita SOL en la bóveda y actualiza la posición.
    pub fn depositar(ctx: Context<Depositar>, monto: u64) -> Result<()> {
        require!(monto > 0, ErrorStaking::MontoCero);
        require!(!ctx.accounts.pool.pausado, ErrorStaking::Pausado);
        let ahora = Clock::get()?.unix_timestamp;
        let tasa = ctx.accounts.pool.tasa_por_segundo;
        let pos = &mut ctx.accounts.posicion;
        if pos.monto == 0 {
            pos.dueno = ctx.accounts.usuario.key();
            pos.desde = ahora;
            pos.ultimo_calculo = ahora;
        }
        pos.pendiente += pos.monto as u128 * tasa as u128 * (ahora - pos.ultimo_calculo).max(0) as u128;
        pos.ultimo_calculo = ahora;
        transfer(CpiContext::new(ctx.accounts.system_program.to_account_info(), Transfer { from: ctx.accounts.usuario.to_account_info(), to: ctx.accounts.boveda.to_account_info() }), monto)?;
        pos.monto = pos.monto.checked_add(monto).ok_or(ErrorStaking::Desbordamiento)?;
        ctx.accounts.pool.total_stakeado += monto;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InicializarPool<'info> {
    #[account(mut)]
    pub autoridad: Signer<'info>,
    #[account(init, payer = autoridad, space = 8 + Pool::INIT_SPACE, seeds = [b"pool", autoridad.key().as_ref()], bump)]
    pub pool: Account<'info, Pool>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Depositar<'info> {
    #[account(mut)]
    pub usuario: Signer<'info>,
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(init_if_needed, payer = usuario, space = 8 + Posicion::INIT_SPACE, seeds = [b"stake", pool.key().as_ref(), usuario.key().as_ref()], bump)]
    pub posicion: Account<'info, Posicion>,
    /// CHECK: PDA validada por seeds; solo recibe lamports.
    #[account(mut, seeds = [b"boveda", pool.key().as_ref()], bump)]
    pub boveda: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct Pool {
    pub autoridad: Pubkey,
    pub tasa_por_segundo: u64,
    pub bloqueo_segundos: i64,
    pub total_stakeado: u64,
    pub pausado: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Posicion {
    pub dueno: Pubkey,
    pub monto: u64,
    pub ultimo_calculo: i64,
    pub desde: i64,
    pub pendiente: u128,
}

#[error_code]
pub enum ErrorStaking {
    #[msg("monto cero")]
    MontoCero,
    #[msg("pool pausado")]
    Pausado,
    #[msg("desbordamiento")]
    Desbordamiento,
}

// Reclamar/Retirar/Fondear/Pausar siguen el mismo patrón; la lógica completa y
// testeada está en ../../../src/programa.rs. ESCALA se usa al pagar recompensas.
#[allow(dead_code)]
fn _escala() -> u128 { ESCALA }
