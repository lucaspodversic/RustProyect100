//! # Contrato de votación descentralizada (proyecto 72)
//!
//! Modelamos el contrato como una máquina de estados determinista, al estilo
//! de un smart contract: cada método recibe el `remitente` (como `msg.sender`)
//! y la `altura` de bloque actual (como `block.number`), valida permisos y
//! fase, modifica estado y **emite eventos**.
//!
//! Funcionalidades:
//! - **Registro** de votantes con peso (sólo el presidente/owner).
//! - **Delegación transitiva** con detección de ciclos (como el `Ballot.sol`
//!   clásico de la documentación de Solidity).
//! - Dos modalidades: **voto público** o **commit-reveal** (secreto hasta el
//!   cierre: primero se publica `sha256(propuesta‖sal‖votante)`, luego se revela).
//! - **Fases** por altura de bloque: registro → votación → revelación → cierre.
//! - **Quórum** mínimo y desempate determinista.
//! - **Votos firmados ed25519** (meta-tx): un relayer envía el voto firmado
//!   por el votante; se usa un `nonce` para impedir repeticiones.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

/// Dirección de cuenta (simplificada a texto).
pub type Direccion = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modalidad {
    Publica,
    CommitReveal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fase {
    Registro,
    Votacion,
    Revelacion,
    Cerrada,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Votante {
    pub peso: u64,
    pub voto: Option<usize>,
    pub delegado_en: Option<Direccion>,
    /// Compromiso pendiente de revelar (modo commit-reveal).
    pub compromiso: Option<[u8; 32]>,
    /// Clave pública ed25519 para votos firmados (opcional).
    pub clave: Option<[u8; 32]>,
    /// Nonce para meta-transacciones.
    pub nonce: u64,
    /// Ya usó su derecho (votó, delegó o comprometió).
    pub uso_derecho: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Propuesta {
    pub nombre: String,
    pub votos: u64,
}

/// Eventos emitidos (como `event` en Solidity) para indexadores/UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evento {
    VotanteRegistrado { votante: Direccion, peso: u64 },
    Delegado { de: Direccion, a: Direccion, peso: u64 },
    Votado { votante: Direccion, propuesta: usize, peso: u64 },
    Comprometido { votante: Direccion },
    Revelado { votante: Direccion, propuesta: usize, peso: u64 },
    Finalizada { ganadora: Option<usize> },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErrorVoto {
    #[error("sólo el presidente puede hacer esto")]
    NoAutorizado,
    #[error("fase incorrecta: se esperaba {0:?}")]
    FaseIncorrecta(Fase),
    #[error("el votante no está registrado o no tiene peso")]
    SinDerecho,
    #[error("ya votó o delegó")]
    YaVoto,
    #[error("propuesta inexistente")]
    PropuestaInvalida,
    #[error("no se puede delegar en uno mismo")]
    AutoDelegacion,
    #[error("delegación circular")]
    CicloDelegacion,
    #[error("operación no disponible en esta modalidad")]
    ModalidadIncorrecta,
    #[error("la revelación no coincide con el compromiso")]
    RevelacionInvalida,
    #[error("firma inválida o nonce repetido")]
    FirmaInvalida,
    #[error("ya registrado")]
    YaRegistrado,
}

/// Configuración de fases por altura de bloque.
#[derive(Debug, Clone, Copy)]
pub struct Calendario {
    /// Desde esta altura se puede votar (antes: registro).
    pub inicio_votacion: u64,
    /// Desde esta altura se revela (sólo commit-reveal) o se cierra.
    pub fin_votacion: u64,
    /// Fin de la revelación (sólo commit-reveal).
    pub fin_revelacion: u64,
}

pub struct ContratoVotacion {
    pub presidente: Direccion,
    pub modalidad: Modalidad,
    pub calendario: Calendario,
    /// Porcentaje mínimo del peso total que debe votar (0–100).
    pub quorum_pct: u64,
    pub votantes: BTreeMap<Direccion, Votante>,
    pub propuestas: Vec<Propuesta>,
    pub eventos: Vec<Evento>,
    pub finalizada: bool,
    /// Suma de pesos registrados (base del quórum). Se mantiene aparte porque
    /// la delegación mueve peso entre votantes y sumarlos contaría doble.
    pub peso_registrado: u64,
}

/// Compromiso de commit-reveal. Incluir al votante impide copiar el
/// compromiso de otro y revelarlo como propio.
pub fn compromiso(propuesta: usize, sal: &[u8], votante: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update((propuesta as u64).to_le_bytes());
    h.update((sal.len() as u64).to_le_bytes());
    h.update(sal);
    h.update(votante.as_bytes());
    h.finalize().into()
}

/// Mensaje canónico que se firma para un voto meta-tx.
pub fn mensaje_voto(id_contrato: &str, votante: &str, propuesta: usize, nonce: u64) -> Vec<u8> {
    format!("votar|{id_contrato}|{votante}|{propuesta}|{nonce}").into_bytes()
}

impl ContratoVotacion {
    /// "Constructor": el remitente queda como presidente y con peso 1.
    pub fn desplegar(presidente: &str, nombres: &[&str], modalidad: Modalidad, calendario: Calendario, quorum_pct: u64) -> Self {
        let mut c = Self {
            presidente: presidente.into(),
            modalidad,
            calendario,
            quorum_pct: quorum_pct.min(100),
            votantes: BTreeMap::new(),
            propuestas: nombres.iter().map(|n| Propuesta { nombre: n.to_string(), votos: 0 }).collect(),
            eventos: vec![],
            finalizada: false,
            peso_registrado: 1,
        };
        c.votantes.insert(presidente.into(), Votante { peso: 1, ..Default::default() });
        c
    }

    pub fn fase(&self, altura: u64) -> Fase {
        let c = &self.calendario;
        if self.finalizada {
            Fase::Cerrada
        } else if altura < c.inicio_votacion {
            Fase::Registro
        } else if altura < c.fin_votacion {
            Fase::Votacion
        } else if self.modalidad == Modalidad::CommitReveal && altura < c.fin_revelacion {
            Fase::Revelacion
        } else {
            Fase::Cerrada
        }
    }

    fn exigir_fase(&self, altura: u64, f: Fase) -> Result<(), ErrorVoto> {
        if self.fase(altura) == f { Ok(()) } else { Err(ErrorVoto::FaseIncorrecta(f)) }
    }

    /// Registra un votante (con clave ed25519 opcional para votos firmados).
    pub fn registrar(&mut self, remitente: &str, votante: &str, peso: u64, clave: Option<[u8; 32]>, altura: u64) -> Result<(), ErrorVoto> {
        if remitente != self.presidente {
            return Err(ErrorVoto::NoAutorizado);
        }
        self.exigir_fase(altura, Fase::Registro)?;
        let v = self.votantes.entry(votante.into()).or_default();
        if v.peso > 0 && votante != self.presidente {
            return Err(ErrorVoto::YaRegistrado);
        }
        let anterior = v.peso;
        v.peso = peso;
        v.clave = clave;
        self.peso_registrado = self.peso_registrado - anterior + peso;
        self.eventos.push(Evento::VotanteRegistrado { votante: votante.into(), peso });
        Ok(())
    }

    /// Delega el voto. Sigue la cadena de delegados hasta el final; si el
    /// delegado final ya votó, suma el peso directamente a su propuesta.
    pub fn delegar(&mut self, remitente: &str, a: &str, altura: u64) -> Result<(), ErrorVoto> {
        self.exigir_fase(altura, Fase::Votacion)?;
        let yo = self.votantes.get(remitente).ok_or(ErrorVoto::SinDerecho)?;
        if yo.peso == 0 {
            return Err(ErrorVoto::SinDerecho);
        }
        if yo.uso_derecho {
            return Err(ErrorVoto::YaVoto);
        }
        if remitente == a {
            return Err(ErrorVoto::AutoDelegacion);
        }
        // Seguimos la cadena: a → a.delegado_en → … ; si volvemos a nosotros, hay ciclo.
        let mut destino = a.to_string();
        let mut pasos = 0;
        while let Some(sig) = self.votantes.get(&destino).and_then(|v| v.delegado_en.clone()) {
            if sig == remitente {
                return Err(ErrorVoto::CicloDelegacion);
            }
            destino = sig;
            pasos += 1;
            if pasos > self.votantes.len() {
                return Err(ErrorVoto::CicloDelegacion);
            }
        }
        if destino == remitente {
            return Err(ErrorVoto::CicloDelegacion);
        }
        let peso = yo.peso;
        // El delegado debe estar registrado (puede tener peso 0 si sólo recibe delegaciones).
        let d = self.votantes.get(&destino).ok_or(ErrorVoto::SinDerecho)?.clone();
        if self.modalidad == Modalidad::CommitReveal && d.compromiso.is_some() {
            // No podemos sumar a un voto secreto ya comprometido.
            return Err(ErrorVoto::YaVoto);
        }
        let y = self.votantes.get_mut(remitente).unwrap();
        y.uso_derecho = true;
        y.delegado_en = Some(destino.clone());
        match d.voto {
            Some(p) => self.propuestas[p].votos += peso,
            None => self.votantes.get_mut(&destino).unwrap().peso += peso,
        }
        self.eventos.push(Evento::Delegado { de: remitente.into(), a: destino, peso });
        Ok(())
    }

    /// Voto público directo.
    pub fn votar(&mut self, remitente: &str, propuesta: usize, altura: u64) -> Result<(), ErrorVoto> {
        if self.modalidad != Modalidad::Publica {
            return Err(ErrorVoto::ModalidadIncorrecta);
        }
        self.exigir_fase(altura, Fase::Votacion)?;
        if propuesta >= self.propuestas.len() {
            return Err(ErrorVoto::PropuestaInvalida);
        }
        let v = self.votantes.get_mut(remitente).ok_or(ErrorVoto::SinDerecho)?;
        if v.peso == 0 {
            return Err(ErrorVoto::SinDerecho);
        }
        if v.uso_derecho {
            return Err(ErrorVoto::YaVoto);
        }
        v.uso_derecho = true;
        v.voto = Some(propuesta);
        let peso = v.peso;
        self.propuestas[propuesta].votos += peso;
        self.eventos.push(Evento::Votado { votante: remitente.into(), propuesta, peso });
        Ok(())
    }

    /// Voto firmado enviado por un tercero (relayer). Se verifica la firma
    /// ed25519 sobre `mensaje_voto(...)` con el nonce actual del votante.
    pub fn votar_firmado(&mut self, votante: &str, propuesta: usize, firma: &[u8; 64], id_contrato: &str, altura: u64) -> Result<(), ErrorVoto> {
        let v = self.votantes.get(votante).ok_or(ErrorVoto::SinDerecho)?;
        let clave = VerifyingKey::from_bytes(&v.clave.ok_or(ErrorVoto::FirmaInvalida)?).map_err(|_| ErrorVoto::FirmaInvalida)?;
        let msg = mensaje_voto(id_contrato, votante, propuesta, v.nonce);
        clave.verify(&msg, &Signature::from_bytes(firma)).map_err(|_| ErrorVoto::FirmaInvalida)?;
        self.votar(votante, propuesta, altura)?;
        self.votantes.get_mut(votante).unwrap().nonce += 1;
        Ok(())
    }

    /// Fase 1 de commit-reveal: publicar el hash del voto.
    pub fn comprometer(&mut self, remitente: &str, hash: [u8; 32], altura: u64) -> Result<(), ErrorVoto> {
        if self.modalidad != Modalidad::CommitReveal {
            return Err(ErrorVoto::ModalidadIncorrecta);
        }
        self.exigir_fase(altura, Fase::Votacion)?;
        let v = self.votantes.get_mut(remitente).ok_or(ErrorVoto::SinDerecho)?;
        if v.peso == 0 {
            return Err(ErrorVoto::SinDerecho);
        }
        if v.uso_derecho {
            return Err(ErrorVoto::YaVoto);
        }
        v.uso_derecho = true;
        v.compromiso = Some(hash);
        self.eventos.push(Evento::Comprometido { votante: remitente.into() });
        Ok(())
    }

    /// Fase 2: revelar propuesta + sal; se suma el peso si coincide.
    pub fn revelar(&mut self, remitente: &str, propuesta: usize, sal: &[u8], altura: u64) -> Result<(), ErrorVoto> {
        self.exigir_fase(altura, Fase::Revelacion)?;
        if propuesta >= self.propuestas.len() {
            return Err(ErrorVoto::PropuestaInvalida);
        }
        let v = self.votantes.get_mut(remitente).ok_or(ErrorVoto::SinDerecho)?;
        let c = v.compromiso.ok_or(ErrorVoto::RevelacionInvalida)?;
        if v.voto.is_some() {
            return Err(ErrorVoto::YaVoto);
        }
        if compromiso(propuesta, sal, remitente) != c {
            return Err(ErrorVoto::RevelacionInvalida);
        }
        v.voto = Some(propuesta);
        let peso = v.peso;
        self.propuestas[propuesta].votos += peso;
        self.eventos.push(Evento::Revelado { votante: remitente.into(), propuesta, peso });
        Ok(())
    }

    /// Cierra la votación: verifica quórum y devuelve la ganadora (empate → menor índice).
    pub fn finalizar(&mut self, altura: u64) -> Result<Option<usize>, ErrorVoto> {
        if self.fase(altura) != Fase::Cerrada {
            return Err(ErrorVoto::FaseIncorrecta(Fase::Cerrada));
        }
        let emitidos: u64 = self.propuestas.iter().map(|p| p.votos).sum();
        let registrado = self.peso_registrado;
        let ganadora = if emitidos * 100 < registrado * self.quorum_pct || emitidos == 0 {
            None
        } else {
            // max_by_key devuelve el ÚLTIMO máximo; recorremos al revés para quedarnos con el primero.
            self.propuestas.iter().enumerate().rev().max_by_key(|(_, p)| p.votos).map(|(i, _)| i)
        };
        self.finalizada = true;
        self.eventos.push(Evento::Finalizada { ganadora });
        Ok(ganadora)
    }
}
