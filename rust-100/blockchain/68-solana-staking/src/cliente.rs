//! Helpers del lado cliente: construyen la lista de cuentas de cada
//! instrucción, como haría el SDK TypeScript generado por Anchor.

use crate::programa::{pda_boveda, pda_pool, pda_posicion, pda_tesoro, procesar, ErrorPrograma, Instruccion};
use crate::runtime::{Banco, MetaCuenta, Pubkey};

pub struct Cliente<'a> {
    pub banco: &'a mut Banco,
    pub pool: Pubkey,
}

impl<'a> Cliente<'a> {
    pub fn inicializar(banco: &'a mut Banco, autoridad: Pubkey, tasa: u64, bloqueo: i64) -> Result<Self, ErrorPrograma> {
        let pool = pda_pool(&autoridad).0;
        procesar(banco, &[MetaCuenta::firmante(autoridad), MetaCuenta::escribible(pool)], &Instruccion::InicializarPool { tasa_por_segundo: tasa, bloqueo_segundos: bloqueo }.empaquetar())?;
        Ok(Self { banco, pool })
    }

    pub fn depositar(&mut self, usuario: Pubkey, monto: u64) -> Result<(), ErrorPrograma> {
        let metas = [MetaCuenta::firmante(usuario), MetaCuenta::escribible(self.pool), MetaCuenta::escribible(pda_posicion(&self.pool, &usuario).0), MetaCuenta::escribible(pda_boveda(&self.pool).0)];
        procesar(self.banco, &metas, &Instruccion::Depositar { monto }.empaquetar())
    }

    pub fn reclamar(&mut self, usuario: Pubkey) -> Result<(), ErrorPrograma> {
        let metas = [MetaCuenta::firmante(usuario), MetaCuenta::lectura(self.pool), MetaCuenta::escribible(pda_posicion(&self.pool, &usuario).0), MetaCuenta::escribible(pda_tesoro(&self.pool).0)];
        procesar(self.banco, &metas, &Instruccion::Reclamar.empaquetar())
    }

    pub fn retirar(&mut self, usuario: Pubkey, monto: u64) -> Result<(), ErrorPrograma> {
        let metas = [MetaCuenta::firmante(usuario), MetaCuenta::escribible(self.pool), MetaCuenta::escribible(pda_posicion(&self.pool, &usuario).0), MetaCuenta::escribible(pda_boveda(&self.pool).0)];
        procesar(self.banco, &metas, &Instruccion::Retirar { monto }.empaquetar())
    }

    pub fn fondear(&mut self, quien: Pubkey, monto: u64) -> Result<(), ErrorPrograma> {
        let metas = [MetaCuenta::firmante(quien), MetaCuenta::lectura(self.pool), MetaCuenta::escribible(pda_tesoro(&self.pool).0)];
        procesar(self.banco, &metas, &Instruccion::Fondear { monto }.empaquetar())
    }

    pub fn pausar(&mut self, autoridad: Pubkey, pausado: bool) -> Result<(), ErrorPrograma> {
        procesar(self.banco, &[MetaCuenta::firmante(autoridad), MetaCuenta::escribible(self.pool)], &Instruccion::Pausar { pausado }.empaquetar())
    }
}
