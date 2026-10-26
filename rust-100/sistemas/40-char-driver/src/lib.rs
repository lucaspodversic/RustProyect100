//! # Driver de dispositivo de caracteres (mock userspace)
//!
//! Simula un *char device* con buffer circular: `open/read/write/ioctl/close`.
//! En Linux real sería un módulo con `register_chrdev` + `file_operations`.
//! Aquí el "kernel" es un registro en memoria y los "procesos" son hilos.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

/// Códigos ioctl educativos.
pub mod ioctl {
    pub const GET_LEN: u64 = 0x1001;
    pub const SET_NONBLOCK: u64 = 0x1002;
    pub const FLUSH: u64 = 0x1003;
}

#[derive(Debug)]
struct State {
    buf: VecDeque<u8>,
    capacity: usize,
    nonblock: bool,
    open_count: usize,
}

/// El dispositivo. Clonable (como un inode compartido).
#[derive(Clone)]
pub struct CharDev {
    st: Arc<(Mutex<State>, Condvar, Condvar)>, // (state, not_empty, not_full)
    pub name: String,
}

impl CharDev {
    pub fn new(name: &str, capacity: usize) -> Self {
        Self {
            name: name.into(),
            st: Arc::new((
                Mutex::new(State { buf: VecDeque::new(), capacity, nonblock: false, open_count: 0 }),
                Condvar::new(),
                Condvar::new(),
            )),
        }
    }

    pub fn open(&self) -> Handle {
        self.st.0.lock().unwrap().open_count += 1;
        Handle { dev: self.clone() }
    }
}

/// Handle abierto (como un `struct file`).
pub struct Handle {
    dev: CharDev,
}

impl Handle {
    pub fn write(&self, data: &[u8]) -> Result<usize, String> {
        let (lock, not_empty, not_full) = &*self.dev.st;
        let mut st = lock.lock().unwrap();
        let mut written = 0;
        for &b in data {
            while st.buf.len() >= st.capacity {
                if st.nonblock {
                    return if written == 0 { Err("EAGAIN".into()) } else { Ok(written) };
                }
                st = not_full.wait(st).unwrap();
            }
            st.buf.push_back(b);
            written += 1;
        }
        not_empty.notify_all();
        Ok(written)
    }

    pub fn read(&self, n: usize) -> Result<Vec<u8>, String> {
        let (lock, not_empty, not_full) = &*self.dev.st;
        let mut st = lock.lock().unwrap();
        while st.buf.is_empty() {
            if st.nonblock {
                return Err("EAGAIN".into());
            }
            st = not_empty.wait(st).unwrap();
        }
        let mut out = Vec::new();
        for _ in 0..n {
            match st.buf.pop_front() {
                Some(b) => out.push(b),
                None => break,
            }
        }
        not_full.notify_all();
        Ok(out)
    }

    pub fn ioctl(&self, cmd: u64, arg: u64) -> Result<u64, String> {
        let (lock, _, _) = &*self.dev.st;
        let mut st = lock.lock().unwrap();
        match cmd {
            ioctl::GET_LEN => Ok(st.buf.len() as u64),
            ioctl::SET_NONBLOCK => { st.nonblock = arg != 0; Ok(0) }
            ioctl::FLUSH => { st.buf.clear(); Ok(0) }
            _ => Err("ENOTTY".into()),
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.dev.st.0.lock().unwrap().open_count -= 1;
    }
}

/// Registro global de dispositivos (como `/dev`).
#[derive(Default)]
pub struct DevRegistry {
    pub devices: Mutex<Vec<CharDev>>,
}

impl DevRegistry {
    pub fn register(&self, dev: CharDev) -> usize {
        let mut g = self.devices.lock().unwrap();
        g.push(dev);
        g.len() - 1 // minor number
    }
    pub fn get(&self, minor: usize) -> Option<CharDev> {
        self.devices.lock().unwrap().get(minor).cloned()
    }
}

#[cfg(feature = "linux")]
pub mod linux_docs {
    //! Path real (esqueleto):
    //! ```ignore
    //! static struct file_operations fops = {
    //!   .owner = THIS_MODULE, .read = my_read, .write = my_write, .unlocked_ioctl = my_ioctl,
    //! };
    //! register_chrdev(MAJOR, "toy", &fops);
    //! ```
    //! En Rust se puede escribir con `kernel` crate (Rust-for-Linux).
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn write_read_basico() {
        let d = CharDev::new("toy", 16);
        let h = d.open();
        assert_eq!(h.write(b"hola").unwrap(), 4);
        assert_eq!(h.ioctl(ioctl::GET_LEN, 0).unwrap(), 4);
        assert_eq!(h.read(2).unwrap(), b"ho");
        assert_eq!(h.read(10).unwrap(), b"la");
    }

    #[test]
    fn bloqueo_y_nonblock() {
        let d = CharDev::new("toy", 2);
        let h = d.open();
        h.ioctl(ioctl::SET_NONBLOCK, 1).unwrap();
        assert_eq!(h.write(b"ab").unwrap(), 2);
        assert_eq!(h.write(b"x").unwrap_err(), "EAGAIN");
        h.ioctl(ioctl::SET_NONBLOCK, 0).unwrap();
        let d2 = d.clone();
        let t = thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            let h = d2.open();
            let _ = h.read(2);
        });
        assert_eq!(h.write(b"cd").unwrap(), 2); // espera a que el otro lea
        t.join().unwrap();
    }

    #[test]
    fn registry() {
        let reg = DevRegistry::default();
        let minor = reg.register(CharDev::new("vowel", 8));
        let h = reg.get(minor).unwrap().open();
        h.write(b"ae").unwrap();
        assert_eq!(h.read(2).unwrap(), b"ae");
    }
}
