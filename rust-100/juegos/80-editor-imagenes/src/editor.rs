//! Editor con historial (deshacer/rehacer) y parser de comandos de texto.

use crate::filtros as f;
use image::RgbaImage;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ErrorEditor {
    #[error("comando desconocido: {0}")]
    Desconocido(String),
    #[error("argumentos inválidos para '{0}'")]
    Argumentos(String),
    #[error("recorte fuera de la imagen")]
    RecorteInvalido,
    #[error("nada para deshacer/rehacer")]
    SinHistorial,
}

/// Operaciones soportadas (parseadas desde texto como `blur:1.5`).
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Grises,
    Invertir,
    Sepia,
    Brillo(i32),
    Contraste(f32),
    Umbral(u8),
    Desenfoque(f32),
    Enfocar,
    Bordes,
    Recortar(u32, u32, u32, u32),
    Redimensionar(u32, u32, bool), // bool: vecino más cercano
    Rotar90,
    EspejoH,
    EspejoV,
}

impl Op {
    /// Sintaxis: `nombre` o `nombre:arg1,arg2,...`.
    pub fn parsear(s: &str) -> Result<Op, ErrorEditor> {
        let (nombre, args) = s.split_once(':').unwrap_or((s, ""));
        let nums = |n: usize| -> Result<Vec<f32>, ErrorEditor> {
            let v: Vec<f32> = args.split(',').filter(|a| !a.is_empty()).map(|a| a.trim().parse::<f32>()).collect::<Result<_, _>>().map_err(|_| ErrorEditor::Argumentos(nombre.into()))?;
            if v.len() != n || v.iter().any(|x| !x.is_finite()) {
                return Err(ErrorEditor::Argumentos(nombre.into()));
            }
            Ok(v)
        };
        let pos = |v: f32| -> Result<u32, ErrorEditor> { if v >= 0.0 && v.fract() == 0.0 && v <= 65_535.0 { Ok(v as u32) } else { Err(ErrorEditor::Argumentos(nombre.into())) } };
        Ok(match nombre {
            "grises" => Op::Grises,
            "invertir" => Op::Invertir,
            "sepia" => Op::Sepia,
            "enfocar" => Op::Enfocar,
            "bordes" => Op::Bordes,
            "rotar" => Op::Rotar90,
            "espejo-h" => Op::EspejoH,
            "espejo-v" => Op::EspejoV,
            "brillo" => Op::Brillo(nums(1)?[0].clamp(-255.0, 255.0) as i32),
            "contraste" => Op::Contraste(nums(1)?[0].clamp(0.0, 10.0)),
            "umbral" => Op::Umbral(nums(1)?[0].clamp(0.0, 255.0) as u8),
            "blur" => Op::Desenfoque(nums(1)?[0].clamp(0.0, 50.0)),
            "recortar" => {
                let v = nums(4)?;
                Op::Recortar(pos(v[0])?, pos(v[1])?, pos(v[2])?, pos(v[3])?)
            }
            "resize" | "resize-px" => {
                let v = nums(2)?;
                let (w, h) = (pos(v[0])?, pos(v[1])?);
                if w == 0 || h == 0 {
                    return Err(ErrorEditor::Argumentos(nombre.into()));
                }
                Op::Redimensionar(w, h, nombre == "resize-px")
            }
            otro => return Err(ErrorEditor::Desconocido(otro.into())),
        })
    }

    pub fn aplicar(&self, img: &RgbaImage) -> Result<RgbaImage, ErrorEditor> {
        Ok(match *self {
            Op::Grises => f::escala_grises(img),
            Op::Invertir => f::invertir(img),
            Op::Sepia => f::sepia(img),
            Op::Brillo(d) => f::brillo(img, d),
            Op::Contraste(k) => f::contraste(img, k),
            Op::Umbral(t) => f::umbral(img, t),
            Op::Desenfoque(s) => f::desenfoque(img, s),
            Op::Enfocar => f::enfocar(img),
            Op::Bordes => f::bordes(img),
            Op::Recortar(x, y, w, h) => f::recortar(img, x, y, w, h).ok_or(ErrorEditor::RecorteInvalido)?,
            Op::Redimensionar(w, h, true) => f::redimensionar_vecino(img, w, h),
            Op::Redimensionar(w, h, false) => f::redimensionar_bilineal(img, w, h),
            Op::Rotar90 => f::rotar90(img),
            Op::EspejoH => f::espejo_h(img),
            Op::EspejoV => f::espejo_v(img),
        })
    }
}

/// Documento abierto + pilas de deshacer/rehacer (con límite de memoria).
pub struct Editor {
    pub imagen: RgbaImage,
    deshacer: Vec<RgbaImage>,
    rehacer: Vec<RgbaImage>,
    pub max_historial: usize,
}

impl Editor {
    pub fn new(imagen: RgbaImage) -> Self {
        Self { imagen, deshacer: vec![], rehacer: vec![], max_historial: 20 }
    }

    /// Aplica una operación; si falla, la imagen queda intacta.
    pub fn aplicar(&mut self, op: &Op) -> Result<(), ErrorEditor> {
        let nueva = op.aplicar(&self.imagen)?;
        self.deshacer.push(std::mem::replace(&mut self.imagen, nueva));
        if self.deshacer.len() > self.max_historial {
            self.deshacer.remove(0);
        }
        self.rehacer.clear(); // una edición nueva invalida el "rehacer"
        Ok(())
    }

    pub fn ejecutar(&mut self, comando: &str) -> Result<(), ErrorEditor> {
        match comando {
            "deshacer" => self.deshacer(),
            "rehacer" => self.rehacer(),
            c => self.aplicar(&Op::parsear(c)?),
        }
    }

    pub fn deshacer(&mut self) -> Result<(), ErrorEditor> {
        let prev = self.deshacer.pop().ok_or(ErrorEditor::SinHistorial)?;
        self.rehacer.push(std::mem::replace(&mut self.imagen, prev));
        Ok(())
    }

    pub fn rehacer(&mut self) -> Result<(), ErrorEditor> {
        let sig = self.rehacer.pop().ok_or(ErrorEditor::SinHistorial)?;
        self.deshacer.push(std::mem::replace(&mut self.imagen, sig));
        Ok(())
    }

    pub fn niveles_historial(&self) -> (usize, usize) {
        (self.deshacer.len(), self.rehacer.len())
    }
}
