//! # Emulador CHIP-8
//!
//! CPU de 8 bits de juguete: 4096 bytes de RAM, 16 registros V0–VF,
//! stack de 16, PC, I, timers. Implementamos el set clásico suficiente
//! para correr ROMs de test (sin GUI: el framebuffer es un array).

pub const WIDTH: usize = 64;
pub const HEIGHT: usize = 32;

pub struct Chip8 {
    pub mem: [u8; 4096],
    pub v: [u8; 16],
    pub i: u16,
    pub pc: u16,
    pub sp: usize,
    pub stack: [u16; 16],
    pub dt: u8, // delay timer
    pub st: u8, // sound timer
    pub fb: [u8; WIDTH * HEIGHT], // 0 o 1
    pub keys: [bool; 16],
    pub draw: bool,
}

impl Chip8 {
    pub fn new() -> Self {
        let mut c = Self {
            mem: [0; 4096],
            v: [0; 16],
            i: 0,
            pc: 0x200,
            sp: 0,
            stack: [0; 16],
            dt: 0,
            st: 0,
            fb: [0; WIDTH * HEIGHT],
            keys: [false; 16],
            draw: false,
        };
        // Fontset clásico en 0x050.
        const FONT: [u8; 80] = [
            0xF0,0x90,0x90,0x90,0xF0, 0x20,0x60,0x20,0x20,0x70,
            0xF0,0x10,0xF0,0x80,0xF0, 0xF0,0x10,0xF0,0x10,0xF0,
            0x90,0x90,0xF0,0x10,0x10, 0xF0,0x80,0xF0,0x10,0xF0,
            0xF0,0x80,0xF0,0x90,0xF0, 0xF0,0x10,0x20,0x40,0x40,
            0xF0,0x90,0xF0,0x90,0xF0, 0xF0,0x90,0xF0,0x10,0xF0,
            0xF0,0x90,0xF0,0x90,0x90, 0xE0,0x90,0xE0,0x90,0xE0,
            0xF0,0x80,0x80,0x80,0xF0, 0xE0,0x90,0x90,0x90,0xE0,
            0xF0,0x80,0xF0,0x80,0xF0, 0xF0,0x80,0xF0,0x80,0x80,
        ];
        c.mem[0x50..0x50 + 80].copy_from_slice(&FONT);
        c
    }

    pub fn load(&mut self, rom: &[u8]) {
        self.mem[0x200..0x200 + rom.len()].copy_from_slice(rom);
    }

    /// Ejecuta una instrucción.
    pub fn step(&mut self) {
        let op = ((self.mem[self.pc as usize] as u16) << 8) | self.mem[self.pc as usize + 1] as u16;
        self.pc += 2;
        let x = ((op & 0x0F00) >> 8) as usize;
        let y = ((op & 0x00F0) >> 4) as usize;
        let n = (op & 0x000F) as u8;
        let nn = (op & 0x00FF) as u8;
        let nnn = op & 0x0FFF;
        match op & 0xF000 {
            0x0000 if op == 0x00E0 => { self.fb.fill(0); self.draw = true; }
            0x0000 if op == 0x00EE => { self.sp -= 1; self.pc = self.stack[self.sp]; }
            0x1000 => self.pc = nnn,
            0x2000 => { self.stack[self.sp] = self.pc; self.sp += 1; self.pc = nnn; }
            0x3000 => if self.v[x] == nn { self.pc += 2; }
            0x4000 => if self.v[x] != nn { self.pc += 2; }
            0x5000 => if self.v[x] == self.v[y] { self.pc += 2; }
            0x6000 => self.v[x] = nn,
            0x7000 => self.v[x] = self.v[x].wrapping_add(nn),
            0x8000 => match n {
                0 => self.v[x] = self.v[y],
                1 => self.v[x] |= self.v[y],
                2 => self.v[x] &= self.v[y],
                3 => self.v[x] ^= self.v[y],
                4 => {
                    let (r, c) = self.v[x].overflowing_add(self.v[y]);
                    self.v[x] = r; self.v[0xF] = c as u8;
                }
                5 => {
                    self.v[0xF] = if self.v[x] >= self.v[y] { 1 } else { 0 };
                    self.v[x] = self.v[x].wrapping_sub(self.v[y]);
                }
                6 => { self.v[0xF] = self.v[x] & 1; self.v[x] >>= 1; }
                7 => {
                    self.v[0xF] = if self.v[y] >= self.v[x] { 1 } else { 0 };
                    self.v[x] = self.v[y].wrapping_sub(self.v[x]);
                }
                0xE => { self.v[0xF] = (self.v[x] >> 7) & 1; self.v[x] <<= 1; }
                _ => {}
            },
            0x9000 => if self.v[x] != self.v[y] { self.pc += 2; }
            0xA000 => self.i = nnn,
            0xB000 => self.pc = nnn + self.v[0] as u16,
            0xC000 => self.v[x] = (self.pc as u8).wrapping_mul(17) & nn, // RNG barato
            0xD000 => {
                // DRW Vx, Vy, nibble
                self.v[0xF] = 0;
                let (vx, vy) = (self.v[x] as usize % WIDTH, self.v[y] as usize % HEIGHT);
                for row in 0..n as usize {
                    let sprite = self.mem[self.i as usize + row];
                    for col in 0..8 {
                        if sprite & (0x80 >> col) != 0 {
                            let px = (vx + col) % WIDTH;
                            let py = (vy + row) % HEIGHT;
                            let idx = py * WIDTH + px;
                            if self.fb[idx] == 1 { self.v[0xF] = 1; }
                            self.fb[idx] ^= 1;
                        }
                    }
                }
                self.draw = true;
            }
            0xE000 if nn == 0x9E => if self.keys[self.v[x] as usize] { self.pc += 2; }
            0xE000 if nn == 0xA1 => if !self.keys[self.v[x] as usize] { self.pc += 2; }
            0xF000 => match nn {
                0x07 => self.v[x] = self.dt,
                0x15 => self.dt = self.v[x],
                0x18 => self.st = self.v[x],
                0x1E => self.i = self.i.wrapping_add(self.v[x] as u16),
                0x29 => self.i = 0x50 + self.v[x] as u16 * 5,
                0x33 => {
                    self.mem[self.i as usize] = self.v[x] / 100;
                    self.mem[self.i as usize + 1] = (self.v[x] / 10) % 10;
                    self.mem[self.i as usize + 2] = self.v[x] % 10;
                }
                0x55 => for i in 0..=x { self.mem[self.i as usize + i] = self.v[i]; }
                0x65 => for i in 0..=x { self.v[i] = self.mem[self.i as usize + i]; }
                _ => {}
            },
            _ => {}
        }
    }

    pub fn tick_timers(&mut self) {
        if self.dt > 0 { self.dt -= 1; }
        if self.st > 0 { self.st -= 1; }
    }
}

impl Default for Chip8 { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_y_add() {
        let mut c = Chip8::new();
        // 6XNN = set, 7XNN = add
        c.load(&[0x60, 0x0A, 0x70, 0x05]); // V0=10; V0+=5
        c.step();
        c.step();
        assert_eq!(c.v[0], 15);
    }
    #[test]
    fn jump_y_call_ret() {
        let mut c = Chip8::new();
        // en 0x200: CALL 0x204; en 0x204: RET  (nunca llega a 0x202)
        c.load(&[0x22, 0x04, 0x00, 0x00, 0x00, 0xEE]);
        c.step(); // call
        assert_eq!(c.pc, 0x204);
        c.step(); // ret
        assert_eq!(c.pc, 0x202);
    }
    #[test]
    fn draw_xor() {
        let mut c = Chip8::new();
        c.i = 0x300;
        c.mem[0x300] = 0x80; // un pixel
        c.v[0] = 0; c.v[1] = 0;
        c.load(&[0xD0, 0x11]); // DRW V0 V1 1
        c.step();
        assert_eq!(c.fb[0], 1);
        assert_eq!(c.v[0xF], 0);
        c.pc = 0x200;
        c.step(); // dibujar de nuevo → colisión
        assert_eq!(c.fb[0], 0);
        assert_eq!(c.v[0xF], 1);
    }
}
