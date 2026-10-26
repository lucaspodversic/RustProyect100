//! Entero sin signo de 256 bits implementado a mano (4 limbs de 64 bits,
//! little-endian: `limbs[0]` es el menos significativo). Todas las
//! operaciones aritméticas son **módulo 2^256**, como en la EVM.

use std::cmp::Ordering;
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct U256(pub [u64; 4]);

impl U256 {
    pub const ZERO: U256 = U256([0; 4]);
    pub const ONE: U256 = U256([1, 0, 0, 0]);
    pub const MAX: U256 = U256([u64::MAX; 4]);

    pub fn from_u64(v: u64) -> Self {
        U256([v, 0, 0, 0])
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0; 4]
    }

    /// Devuelve `Some(u64)` si el valor entra en 64 bits.
    pub fn as_u64(&self) -> Option<u64> {
        (self.0[1] == 0 && self.0[2] == 0 && self.0[3] == 0).then_some(self.0[0])
    }

    /// Como `usize` saturando (útil para offsets de memoria: valores enormes → error de gas).
    pub fn as_usize_sat(&self) -> usize {
        self.as_u64().map(|v| v.min(usize::MAX as u64) as usize).unwrap_or(usize::MAX)
    }

    pub fn from_be_bytes(b: &[u8]) -> Self {
        // Acepta hasta 32 bytes, alineados a la derecha.
        let mut buf = [0u8; 32];
        let n = b.len().min(32);
        buf[32 - n..].copy_from_slice(&b[b.len() - n..]);
        let mut l = [0u64; 4];
        for (i, limb) in l.iter_mut().enumerate() {
            let ini = 32 - (i + 1) * 8;
            *limb = u64::from_be_bytes(buf[ini..ini + 8].try_into().unwrap());
        }
        U256(l)
    }

    pub fn to_be_bytes(&self) -> [u8; 32] {
        let mut o = [0u8; 32];
        for i in 0..4 {
            let ini = 32 - (i + 1) * 8;
            o[ini..ini + 8].copy_from_slice(&self.0[i].to_be_bytes());
        }
        o
    }

    pub fn overflowing_add(self, o: Self) -> (Self, bool) {
        let mut r = [0u64; 4];
        let mut carry = false;
        for (i, ri) in r.iter_mut().enumerate() {
            let (s1, c1) = self.0[i].overflowing_add(o.0[i]);
            let (s2, c2) = s1.overflowing_add(carry as u64);
            *ri = s2;
            carry = c1 || c2;
        }
        (U256(r), carry)
    }

    pub fn wrapping_add(self, o: Self) -> Self {
        self.overflowing_add(o).0
    }

    pub fn wrapping_sub(self, o: Self) -> Self {
        // a - b = a + (!b + 1) en complemento a dos.
        self.wrapping_add((!o).wrapping_add(U256::ONE))
    }

    /// Multiplicación escolar limb a limb, descartando lo que pasa de 256 bits.
    pub fn wrapping_mul(self, o: Self) -> Self {
        let mut r = [0u64; 4];
        for i in 0..4 {
            let mut carry: u128 = 0;
            for j in 0..(4 - i) {
                let cur = r[i + j] as u128 + (self.0[i] as u128) * (o.0[j] as u128) + carry;
                r[i + j] = cur as u64;
                carry = cur >> 64;
            }
        }
        U256(r)
    }

    pub fn bit(&self, i: usize) -> bool {
        (self.0[i / 64] >> (i % 64)) & 1 == 1
    }

    pub fn bits(&self) -> usize {
        for i in (0..4).rev() {
            if self.0[i] != 0 {
                return i * 64 + 64 - self.0[i].leading_zeros() as usize;
            }
        }
        0
    }

    pub fn shl(self, n: usize) -> Self {
        if n >= 256 {
            return U256::ZERO;
        }
        let (limbs, bits) = (n / 64, n % 64);
        let mut r = [0u64; 4];
        for i in (limbs..4).rev() {
            r[i] = self.0[i - limbs] << bits;
            if bits > 0 && i > limbs {
                r[i] |= self.0[i - limbs - 1] >> (64 - bits);
            }
        }
        U256(r)
    }

    pub fn shr(self, n: usize) -> Self {
        if n >= 256 {
            return U256::ZERO;
        }
        let (limbs, bits) = (n / 64, n % 64);
        let mut r = [0u64; 4];
        for i in 0..(4 - limbs) {
            r[i] = self.0[i + limbs] >> bits;
            if bits > 0 && i + limbs + 1 < 4 {
                r[i] |= self.0[i + limbs + 1] << (64 - bits);
            }
        }
        U256(r)
    }

    /// División larga binaria. En la EVM, dividir por cero da 0 (no error).
    pub fn div_rem(self, d: Self) -> (Self, Self) {
        if d.is_zero() {
            return (U256::ZERO, U256::ZERO);
        }
        let mut q = U256::ZERO;
        let mut r = U256::ZERO;
        for i in (0..self.bits()).rev() {
            r = r.shl(1);
            if self.bit(i) {
                r.0[0] |= 1;
            }
            if r >= d {
                r = r.wrapping_sub(d);
                q.0[i / 64] |= 1 << (i % 64);
            }
        }
        (q, r)
    }

    /// Exponenciación por cuadrados (mod 2^256).
    pub fn wrapping_pow(self, mut e: Self) -> Self {
        let mut base = self;
        let mut acc = U256::ONE;
        while !e.is_zero() {
            if e.bit(0) {
                acc = acc.wrapping_mul(base);
            }
            base = base.wrapping_mul(base);
            e = e.shr(1);
        }
        acc
    }
}

impl std::ops::Not for U256 {
    type Output = U256;
    fn not(self) -> U256 {
        U256(self.0.map(|x| !x))
    }
}

macro_rules! bitop {
    ($tr:ident, $f:ident, $op:tt) => {
        impl std::ops::$tr for U256 {
            type Output = U256;
            fn $f(self, o: U256) -> U256 {
                U256([self.0[0] $op o.0[0], self.0[1] $op o.0[1], self.0[2] $op o.0[2], self.0[3] $op o.0[3]])
            }
        }
    };
}
bitop!(BitAnd, bitand, &);
bitop!(BitOr, bitor, |);
bitop!(BitXor, bitxor, ^);

impl Ord for U256 {
    fn cmp(&self, o: &Self) -> Ordering {
        // Comparamos desde el limb más significativo.
        for i in (0..4).rev() {
            match self.0[i].cmp(&o.0[i]) {
                Ordering::Equal => continue,
                x => return x,
            }
        }
        Ordering::Equal
    }
}
impl PartialOrd for U256 {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl fmt::Debug for U256 {
    /// Formato hexadecimal compacto: `0x0`, `0xff`, ...
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let h = hex::encode(self.to_be_bytes());
        let t = h.trim_start_matches('0');
        write!(f, "0x{}", if t.is_empty() { "0" } else { t })
    }
}

impl From<u64> for U256 {
    fn from(v: u64) -> Self {
        U256::from_u64(v)
    }
}
