//! # Editor de imágenes (proyecto 80)
//!
//! - [`filtros`]: grises, sepia, invertir, brillo, contraste, umbral,
//!   desenfoque gaussiano separable, enfoque, Sobel, recorte, resize
//!   (vecino y bilineal), rotación, espejos e histograma.
//! - [`editor`]: historial deshacer/rehacer y parser de comandos.
//!
//! El crate `image` sólo se usa para el tipo `RgbaImage` y la E/S PNG.

pub mod editor;
pub mod filtros;

/// Imagen de prueba: degradé + cuadrado + círculo (para la demo y tests).
pub fn imagen_demo(w: u32, h: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(w, h, |x, y| {
        let (cx, cy) = (w as f32 * 0.65, h as f32 * 0.5);
        let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
        if d < h as f32 * 0.25 {
            image::Rgba([230, 60, 50, 255])
        } else if x > w / 8 && x < w / 3 && y > h / 4 && y < h * 3 / 4 {
            image::Rgba([40, 90, 220, 255])
        } else {
            image::Rgba([(x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8, 120, 255])
        }
    })
}
