//! Generación de las permutaciones de cada movimiento a partir de geometría.
//!
//! En vez de escribir a mano 18 tablas (fácil equivocarse), modelamos cada
//! sticker como (posición, normal) en una grilla 3D con coordenadas en
//! {-1,0,1}: x→derecha (R), y→arriba (U), z→frente (F). Girar una capa es
//! rotar 90° los stickers de esa capa; buscando a dónde fue cada uno
//! obtenemos la permutación.

pub type V3 = [i32; 3];

/// Orden de caras: U R F D L B (convención estándar).
pub const NORMALES: [V3; 6] = [[0, 1, 0], [1, 0, 0], [0, 0, 1], [0, -1, 0], [-1, 0, 0], [0, 0, -1]];

/// Ejes (derecha, abajo) de cada cara vista desde afuera en la red desplegada.
const EJES: [(V3, V3); 6] = [
    ([1, 0, 0], [0, 0, 1]),   // U: F queda abajo
    ([0, 0, -1], [0, -1, 0]), // R
    ([1, 0, 0], [0, -1, 0]),  // F
    ([1, 0, 0], [0, 0, -1]),  // D: F queda arriba
    ([0, 0, 1], [0, -1, 0]),  // L
    ([-1, 0, 0], [0, -1, 0]), // B
];

fn suma(a: V3, b: V3, k: i32, c: V3, m: i32) -> V3 {
    [a[0] + b[0] * k + c[0] * m, a[1] + b[1] * k + c[1] * m, a[2] + b[2] * k + c[2] * m]
}

fn cruz(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn punto(a: V3, b: V3) -> i32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Rotación de −90° (horario visto desde la punta de `eje`) — Rodrigues
/// con cos = 0, sin = −1:  v' = −(eje × v) + eje·(eje·v).
pub fn rotar_horario(v: V3, eje: V3) -> V3 {
    let c = cruz(eje, v);
    let d = punto(eje, v);
    [-c[0] + eje[0] * d, -c[1] + eje[1] * d, -c[2] + eje[2] * d]
}

/// (posición, normal) del sticker `i` (0..54): índice = cara*9 + fila*3 + col.
pub fn sticker(i: usize) -> (V3, V3) {
    let (cara, fila, col) = (i / 9, (i % 9) / 3, i % 3);
    let n = NORMALES[cara];
    let (der, abajo) = EJES[cara];
    (suma(n, der, col as i32 - 1, abajo, fila as i32 - 1), n)
}

pub fn indice(pos: V3, normal: V3) -> usize {
    (0..54).find(|&i| sticker(i) == (pos, normal)).expect("sticker inexistente")
}

/// Permutación de un cuarto de vuelta horario alrededor de `eje` aplicado a
/// los stickers cuya posición cumple `en_capa`. `p[i] = j` ⇒ el sticker que
/// estaba en `i` pasa a `j`.
pub fn permutacion(eje: V3, en_capa: impl Fn(V3) -> bool) -> [u8; 54] {
    let mut p = [0u8; 54];
    for (i, dest) in p.iter_mut().enumerate() {
        let (pos, n) = sticker(i);
        *dest = if en_capa(pos) { indice(rotar_horario(pos, eje), rotar_horario(n, eje)) } else { i } as u8;
    }
    p
}

/// Producto punto público (lo usa `cubo` para elegir la capa).
pub fn punto3(a: V3, b: V3) -> i32 {
    punto(a, b)
}
