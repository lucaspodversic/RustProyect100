//! Filtros y transformaciones implementados a mano sobre `RgbaImage`.
//! Todas las funciones son puras: reciben una imagen y devuelven otra.

use image::{Rgba, RgbaImage};

/// Luminancia perceptual (Rec. 601).
fn luma(p: &Rgba<u8>) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

fn sat(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// Aplica `f` a cada píxel conservando el alfa.
fn mapear(img: &RgbaImage, f: impl Fn(&Rgba<u8>) -> [f32; 3]) -> RgbaImage {
    let mut out = img.clone();
    for p in out.pixels_mut() {
        let [r, g, b] = f(p);
        *p = Rgba([sat(r), sat(g), sat(b), p[3]]);
    }
    out
}

pub fn escala_grises(img: &RgbaImage) -> RgbaImage {
    mapear(img, |p| {
        let l = luma(p);
        [l, l, l]
    })
}

pub fn invertir(img: &RgbaImage) -> RgbaImage {
    mapear(img, |p| [255.0 - p[0] as f32, 255.0 - p[1] as f32, 255.0 - p[2] as f32])
}

pub fn sepia(img: &RgbaImage) -> RgbaImage {
    mapear(img, |p| {
        let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
        [0.393 * r + 0.769 * g + 0.189 * b, 0.349 * r + 0.686 * g + 0.168 * b, 0.272 * r + 0.534 * g + 0.131 * b]
    })
}

/// Suma `delta` (−255..255) a cada canal.
pub fn brillo(img: &RgbaImage, delta: i32) -> RgbaImage {
    mapear(img, |p| [0, 1, 2].map(|c| p[c] as f32 + delta as f32))
}

/// Contraste: factor > 1 aumenta, < 1 reduce (pivote en 128).
pub fn contraste(img: &RgbaImage, factor: f32) -> RgbaImage {
    mapear(img, |p| [0, 1, 2].map(|c| (p[c] as f32 - 128.0) * factor + 128.0))
}

/// Umbral binario sobre la luminancia.
pub fn umbral(img: &RgbaImage, t: u8) -> RgbaImage {
    mapear(img, |p| {
        let v = if luma(p) >= t as f32 { 255.0 } else { 0.0 };
        [v, v, v]
    })
}

/// Desenfoque gaussiano **separable**: un pase horizontal y uno vertical
/// (O(r) por píxel en vez de O(r²)). Bordes: se repite el píxel del borde.
pub fn desenfoque(img: &RgbaImage, sigma: f32) -> RgbaImage {
    if sigma <= 0.0 {
        return img.clone();
    }
    let r = (sigma * 3.0).ceil() as i32;
    let mut k: Vec<f32> = (-r..=r).map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp()).collect();
    let s: f32 = k.iter().sum();
    k.iter_mut().for_each(|v| *v /= s);
    let pase = |src: &RgbaImage, horiz: bool| {
        let (w, h) = src.dimensions();
        RgbaImage::from_fn(w, h, |x, y| {
            let mut acc = [0f32; 4];
            for (i, kv) in k.iter().enumerate() {
                let d = i as i32 - r;
                let (sx, sy) = if horiz {
                    ((x as i32 + d).clamp(0, w as i32 - 1) as u32, y)
                } else {
                    (x, (y as i32 + d).clamp(0, h as i32 - 1) as u32)
                };
                let p = src.get_pixel(sx, sy);
                for c in 0..4 {
                    acc[c] += p[c] as f32 * kv;
                }
            }
            Rgba(acc.map(sat))
        })
    };
    pase(&pase(img, true), false)
}

/// Convolución 3×3 genérica sobre RGB (alfa intacto).
pub fn convolucion3(img: &RgbaImage, k: [[f32; 3]; 3]) -> RgbaImage {
    let (w, h) = img.dimensions();
    RgbaImage::from_fn(w, h, |x, y| {
        let mut acc = [0f32; 3];
        for (dy, fila) in k.iter().enumerate() {
            for (dx, kv) in fila.iter().enumerate() {
                let sx = (x as i32 + dx as i32 - 1).clamp(0, w as i32 - 1) as u32;
                let sy = (y as i32 + dy as i32 - 1).clamp(0, h as i32 - 1) as u32;
                let p = img.get_pixel(sx, sy);
                for c in 0..3 {
                    acc[c] += p[c] as f32 * kv;
                }
            }
        }
        Rgba([sat(acc[0]), sat(acc[1]), sat(acc[2]), img.get_pixel(x, y)[3]])
    })
}

pub fn enfocar(img: &RgbaImage) -> RgbaImage {
    convolucion3(img, [[0.0, -1.0, 0.0], [-1.0, 5.0, -1.0], [0.0, -1.0, 0.0]])
}

/// Detección de bordes Sobel: magnitud del gradiente de la luminancia.
pub fn bordes(img: &RgbaImage) -> RgbaImage {
    let g = escala_grises(img);
    let (w, h) = g.dimensions();
    let at = |x: i32, y: i32| g.get_pixel(x.clamp(0, w as i32 - 1) as u32, y.clamp(0, h as i32 - 1) as u32)[0] as f32;
    RgbaImage::from_fn(w, h, |x, y| {
        let (x, y) = (x as i32, y as i32);
        let gx = -at(x - 1, y - 1) - 2.0 * at(x - 1, y) - at(x - 1, y + 1) + at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1);
        let gy = -at(x - 1, y - 1) - 2.0 * at(x, y - 1) - at(x + 1, y - 1) + at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1);
        let m = sat((gx * gx + gy * gy).sqrt());
        Rgba([m, m, m, 255])
    })
}

/// Recorte; `None` si el rectángulo se sale de la imagen o es vacío.
pub fn recortar(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> Option<RgbaImage> {
    let (iw, ih) = img.dimensions();
    if w == 0 || h == 0 || x.checked_add(w)? > iw || y.checked_add(h)? > ih {
        return None;
    }
    Some(RgbaImage::from_fn(w, h, |i, j| *img.get_pixel(x + i, y + j)))
}

/// Redimensionado por vecino más cercano (ideal para pixel art).
pub fn redimensionar_vecino(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    let (iw, ih) = img.dimensions();
    RgbaImage::from_fn(w, h, |x, y| *img.get_pixel((x as u64 * iw as u64 / w as u64) as u32, (y as u64 * ih as u64 / h as u64) as u32))
}

/// Redimensionado bilineal con alineación de centros de píxel.
pub fn redimensionar_bilineal(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    let (iw, ih) = img.dimensions();
    RgbaImage::from_fn(w, h, |x, y| {
        // Centro del píxel destino mapeado al espacio origen.
        let fx = ((x as f32 + 0.5) * iw as f32 / w as f32 - 0.5).clamp(0.0, iw as f32 - 1.0);
        let fy = ((y as f32 + 0.5) * ih as f32 / h as f32 - 0.5).clamp(0.0, ih as f32 - 1.0);
        let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(iw - 1), (y0 + 1).min(ih - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let mut out = [0u8; 4];
        for (c, o) in out.iter_mut().enumerate() {
            let p = |a: u32, b: u32| img.get_pixel(a, b)[c] as f32;
            let arriba = p(x0, y0) * (1.0 - tx) + p(x1, y0) * tx;
            let abajo = p(x0, y1) * (1.0 - tx) + p(x1, y1) * tx;
            *o = sat(arriba * (1.0 - ty) + abajo * ty);
        }
        Rgba(out)
    })
}

pub fn rotar90(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    RgbaImage::from_fn(h, w, |x, y| *img.get_pixel(y, h - 1 - x))
}

pub fn espejo_h(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    RgbaImage::from_fn(w, h, |x, y| *img.get_pixel(w - 1 - x, y))
}

pub fn espejo_v(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    RgbaImage::from_fn(w, h, |x, y| *img.get_pixel(x, h - 1 - y))
}

/// Histograma de luminancia (256 cubetas).
pub fn histograma(img: &RgbaImage) -> [u32; 256] {
    let mut hst = [0u32; 256];
    for p in img.pixels() {
        hst[sat(luma(p)) as usize] += 1;
    }
    hst
}
