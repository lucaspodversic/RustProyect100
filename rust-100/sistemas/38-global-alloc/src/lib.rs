//! # Asignador de memoria global personalizado
//!
//! Un *bump allocator* thread-safe sobre un bloque fijo, con:
//! - Alineación correcta (`align_offset`).
//! - Contadores de alloc/dealloc (estadísticas).
//! - Modo "free-list" simple para reutilizar huecos exactos.
//!
//! **No** lo instalamos como `#[global_allocator]` por defecto: eso rompe
//! el harness de tests. Se puede activar con `--features install`.

use std::alloc::{GlobalAlloc, Layout};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Tamaño del arena (1 MiB). Suficiente para demos; no para producción.
const ARENA: usize = 1 << 20;

/// Entrada libre: offset + tamaño.
#[derive(Clone, Copy, Debug)]
struct Free { offset: usize, size: usize }

/// Estado interno protegido por mutex.
struct Inner {
    /// Arena en el heap del sistema (la pedimos una sola vez a `System`).
    base: *mut u8,
    bump: usize,
    free: Vec<Free>,
}

// Safety: el acceso a `base` está serializado por el Mutex.
unsafe impl Send for Inner {}

/// El asignador. También expone estadísticas atómicas.
pub struct BumpAlloc {
    inner: Mutex<Option<Inner>>,
    pub allocs: AtomicUsize,
    pub deallocs: AtomicUsize,
    pub bytes_live: AtomicUsize,
    pub bytes_total: AtomicUsize,
}

impl BumpAlloc {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            allocs: AtomicUsize::new(0),
            deallocs: AtomicUsize::new(0),
            bytes_live: AtomicUsize::new(0),
            bytes_total: AtomicUsize::new(0),
        }
    }

    fn with_inner<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> R {
        let mut g = self.inner.lock().unwrap();
        if g.is_none() {
            // Pedimos el arena al asignador del sistema una sola vez.
            let layout = Layout::from_size_align(ARENA, 16).unwrap();
            let base = unsafe { std::alloc::System.alloc(layout) };
            assert!(!base.is_null(), "no se pudo reservar el arena");
            *g = Some(Inner { base, bump: 0, free: Vec::new() });
        }
        f(g.as_mut().unwrap())
    }

    /// Estadísticas legibles.
    pub fn stats(&self) -> (usize, usize, usize, usize) {
        (
            self.allocs.load(Ordering::Relaxed),
            self.deallocs.load(Ordering::Relaxed),
            self.bytes_live.load(Ordering::Relaxed),
            self.bytes_total.load(Ordering::Relaxed),
        )
    }
}

unsafe impl GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(1);
        let align = layout.align();
        let ptr = self.with_inner(|inn| {
            // 1) ¿Hay un hueco exacto en la free-list?
            if let Some(i) = inn.free.iter().position(|f| f.size == size && (inn.base as usize + f.offset) % align == 0)
            {
                let f = inn.free.swap_remove(i);
                return inn.base.add(f.offset);
            }
            // 2) Bump: avanzar respetando alineación.
            let cur = inn.base as usize + inn.bump;
            let aligned = (cur + align - 1) & !(align - 1);
            let offset = aligned - inn.base as usize;
            if offset + size > ARENA {
                return null_mut();
            }
            inn.bump = offset + size;
            inn.base.add(offset)
        });
        if !ptr.is_null() {
            self.allocs.fetch_add(1, Ordering::Relaxed);
            self.bytes_live.fetch_add(size, Ordering::Relaxed);
            self.bytes_total.fetch_add(size, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size().max(1);
        self.with_inner(|inn| {
            let offset = ptr as usize - inn.base as usize;
            inn.free.push(Free { offset, size });
        });
        self.deallocs.fetch_add(1, Ordering::Relaxed);
        self.bytes_live.fetch_sub(size, Ordering::Relaxed);
    }
}

/// Instancia global (usable como `#[global_allocator]` si se activa el feature).
pub static ALLOC: BumpAlloc = BumpAlloc::new();

#[cfg(feature = "install")]
#[global_allocator]
static GA: BumpAlloc = BumpAlloc::new();

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Layout;

    #[test]
    fn alloc_dealloc_y_reuso() {
        let a = BumpAlloc::new();
        let layout = Layout::from_size_align(64, 8).unwrap();
        unsafe {
            let p1 = a.alloc(layout);
            assert!(!p1.is_null());
            assert_eq!(p1 as usize % 8, 0);
            a.dealloc(p1, layout);
            let p2 = a.alloc(layout);
            assert_eq!(p1, p2, "reusa el hueco exacto");
        }
        let (al, de, live, _) = a.stats();
        assert_eq!(al, 2);
        assert_eq!(de, 1);
        assert_eq!(live, 64);
    }

    #[test]
    fn alineacion_grande() {
        let a = BumpAlloc::new();
        let layout = Layout::from_size_align(16, 256).unwrap();
        unsafe {
            let p = a.alloc(layout);
            assert!(!p.is_null());
            assert_eq!(p as usize % 256, 0);
            a.dealloc(p, layout);
        }
    }

    #[test]
    fn agota_arena() {
        let a = BumpAlloc::new();
        let layout = Layout::from_size_align(ARENA / 2, 8).unwrap();
        unsafe {
            assert!(!a.alloc(layout).is_null());
            assert!(!a.alloc(layout).is_null());
            assert!(a.alloc(layout).is_null(), "tercera no cabe");
        }
    }
}
