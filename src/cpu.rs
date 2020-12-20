use std::fmt;
use std::time::Instant;

pub const WIDTH: usize = 64;
pub const HEIGHT: usize = 32;

pub struct Timer {
    start: Instant,
    last_update: Instant,
    freq_hz: f64,
    multi: f64,
    _reg_value: u8,
}

impl Timer {
    fn new() -> Self {
        let time = Instant::now();
        Self {
            start: time,
            last_update: time,
            freq_hz: 60.0,
            multi: 1.0,
            _reg_value: 0,
        }
    }

    fn set_reg(&mut self, val: u8) {
        self.last_update = Instant::now();
        self._reg_value = val;
    }

    fn get_reg(&self) -> u8 {
        if self._reg_value == 0 {
            return 0;
        }
        let until_now = Instant::now() - self.start;
        let until_last_update = self.last_update - self.start;
        let steps_now = until_now.as_secs_f64() * self.freq_hz * self.multi;
        let steps_last_update = until_last_update.as_secs_f64() * self.freq_hz * self.multi;
        // cast to int to make the divisions above integer divisions
        let diff = steps_now as u64 - steps_last_update as u64;
        return if (self._reg_value as u64) < diff {
            0
        } else {
            self._reg_value - diff as u8
        };
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum VKey {
    Up,
    Down,
}

#[derive(Debug)]
pub struct Keyboard {
    pub keys: [VKey; 16],
    pub prev_keys: [VKey; 16],
}

impl Default for Keyboard {
    fn default() -> Self {
        Self {
            keys: [VKey::Up; 16],
            prev_keys: [VKey::Up; 16],
        }
    }
}

pub struct Display {
    // x: 0 - 63 pixels or 0-7 bytes
    // y: 0 - 31 bytes
    pub cells: [[u8; WIDTH / 8]; HEIGHT],
    pub updates: u64,
    pub updated: bool,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            cells: [[0u8; WIDTH / 8]; HEIGHT],
            updates: 0,
            updated: true,
        }
    }
}

pub fn display_cells_to_buf(cells: [[u8; WIDTH / 8]; HEIGHT]) -> Vec<u8> {
    let mut buf = Vec::<u8>::with_capacity(WIDTH * HEIGHT);
    for y in 0..HEIGHT {
        for x in 0..WIDTH / 8 {
            for bit in 0..8 {
                if ((cells[y][x] >> (7 - bit)) & 0x1) == 1 {
                    buf.push(1);
                } else {
                    buf.push(0);
                }
            }
        }
    }
    buf
}

impl Display {
    fn clear(&mut self) {
        for y in 0..HEIGHT {
            for x in 0..WIDTH / 8 {
                self.cells[y][x] = 0;
            }
        }
        self.updates += 1
    }

    fn write_sprite(&mut self, sprite: &[u8], x: u8, y: u8) -> bool {
        let mut collision = false;
        let x = x % WIDTH as u8;
        let y = y % HEIGHT as u8;
        for i in 0..sprite.len() {
            let y_roll = ((y as usize + i) % HEIGHT) as u8;
            let cur_val = self.get_byte(x, y_roll);
            let new_val = cur_val ^ sprite[i];
            self.set_byte(x, y_roll, new_val);
            if new_val != cur_val {
                collision = true;
            }
        }
        self.updates += 1;
        self.updated = true;
        collision
    }

    fn get_byte(&mut self, x: u8, y: u8) -> u8 {
        let offs_bytes = x as usize / 8;
        let offs_bits = x as usize % 8;
        let line = &self.cells[y as usize];
        let word = (line[offs_bytes] as u16) << 8 | line[(offs_bytes + 1) % (WIDTH / 8)] as u16;
        let res = ((word >> (8 - offs_bits)) & 0xFF) as u8;
        res
    }

    fn set_byte(&mut self, x: u8, y: u8, val: u8) {
        let offs_bytes = x as usize / 8;
        let offs_bits = x as usize % 8;
        let line = &mut self.cells[y as usize];

        let mut word = (line[offs_bytes] as u16) << 8 | line[(offs_bytes + 1) % (WIDTH / 8)] as u16;
        word &= !(0xFF << (8 - offs_bits));
        word |= (val as u16) << (8 - offs_bits);
        line[offs_bytes] = ((word >> 8) & 0xFF) as u8;
        line[(offs_bytes + 1) % 8] = (word & 0xFF) as u8;
    }

    fn std_sprites(&self) -> [u8; 80] {
        [
            0xF0, 0x90, 0x90, 0x90, 0xF0, 0x20, 0x60, 0x20, 0x20, 0x70, 0xF0, 0x10, 0xF0, 0x80,
            0xF0, 0xF0, 0x10, 0xF0, 0x10, 0xF0, 0x90, 0x90, 0xF0, 0x10, 0x10, 0xF0, 0x80, 0xF0,
            0x10, 0xF0, 0xF0, 0x80, 0xF0, 0x90, 0xF0, 0xF0, 0x10, 0x20, 0x40, 0x40, 0xF0, 0x90,
            0xF0, 0x90, 0xF0, 0xF0, 0x90, 0xF0, 0x10, 0xF0, 0xF0, 0x90, 0xF0, 0x90, 0x90, 0xE0,
            0x90, 0xE0, 0x90, 0xE0, 0xF0, 0x80, 0x80, 0x80, 0xF0, 0xE0, 0x90, 0x90, 0x90, 0xE0,
            0xF0, 0x80, 0xF0, 0x80, 0xF0, 0xF0, 0x80, 0xF0, 0x80, 0x80,
        ]
    }
}

pub struct Cpu {
    pub display: Display,
    pub keyboard: Keyboard,
    pub dt: Timer,
    pub st: Timer,
    pub memory: [u8; 4096],
    pub v: [u8; 16],
    pub pc: u16,
    pub sp: u8,
    pub stack: [u16; 16],
    pub i: u16,
    pub clock_steps: u64,
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            display: Display::default(),
            keyboard: Keyboard::default(),
            dt: Timer::new(),
            st: Timer::new(),
            memory: [0u8; 4096],
            v: [0u8; 16],
            pc: 0x200,
            sp: 0,
            stack: [0u16; 16],
            i: 0,
            clock_steps: 0,
        }
    }
}

impl fmt::Debug for Cpu {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Foo")
            .field("pc", &format_args!("{:#X}", self.pc))
            .field("i", &format_args!("{:#X}", self.i))
            .field("dt", &self.dt.get_reg())
            .field("v", &self.v)
            .field("sp", &self.sp)
            .field("stack", &self.stack)
            //.field("addr", &format_args!("{}", self.addr))
            .finish()
    }
}

impl Cpu {
    pub fn new(code: &[u8], multi: f64) -> Self {
        let mut cpu = Self::default();
        cpu.dt.multi = multi;
        cpu.st.multi = multi;
        cpu.memory[0..80].copy_from_slice(&cpu.display.std_sprites());
        cpu.memory[0x200..0x200 + code.len()].copy_from_slice(code);
        cpu.pc = 0x200;
        cpu
    }

    pub fn next_instruction(&self) -> u16 {
        read_memory(&self.memory, self.pc)
    }

    pub fn tick(&mut self) -> u16 {
        let instr = self.next_instruction();
        self.process_instruction(instr);
        self.clock_steps += 1;
        //assert!((self.pc % 2) == 0, "program counter is not even");
        // slipperyslope jumps to uneven instruction (level-unpack at 0x265 (0x65 in file))
        instr
    }

    fn process_instruction(&mut self, instr: u16) {
        let mut nibbles = [0u8; 4];
        nibbles[0] = ((instr >> 12) & 0xF) as u8;
        nibbles[1] = ((instr >> 8) & 0xF) as u8;
        nibbles[2] = ((instr >> 4) & 0xF) as u8;
        nibbles[3] = ((instr >> 0) & 0xF) as u8;
        let x = nibbles[1] as usize;
        let y = nibbles[2] as usize;
        let nnn = instr & 0xFFF;
        let kk: u8 = (instr & 0xFF) as u8;

        match (nibbles[0], nibbles[1], nibbles[2], nibbles[3]) {
            (0x0, _, 0xE, 0x0) => {
                self.display.clear();
            }
            (0x0, _, 0xE, 0xE) => {
                self.pc = self.stack[self.sp as usize];
                self.sp = self.sp - 1;
            }
            (0x1, ..) => {
                // JP addr
                self.pc = nnn;
                return;
            }
            (0x2, ..) => {
                // CALL addr
                self.sp = self.sp + 1;
                self.stack[self.sp as usize] = self.pc;
                self.pc = nnn;
                return;
            }
            (0x3, ..) => {
                // SE Vx, byte
                if self.v[x] == kk {
                    self.pc = self.pc + 4;
                    return;
                }
            }
            (0x4, ..) => {
                // SNE Vx, byte
                if self.v[x] != kk {
                    self.pc = self.pc + 4;
                    return;
                }
            }
            (0x5, ..) => {
                // 5xy0 - SE Vx, Vy
                if self.v[x] == self.v[y] {
                    self.pc = self.pc + 4;
                    return;
                }
            }
            (0x6, ..) => {
                // 6xkk - LD Vx, byte
                self.v[x] = kk;
            }
            (0x7, ..) => {
                // 7xkk - ADD Vx, byte
                let tmp = self.v[x] as u16 + kk as u16;
                self.v[x] = (tmp & 0xFF) as u8;
            }
            (0x8, _, _, 0x0) => {
                // 8xy0 - LD Vx, Vy
                self.v[x] = self.v[y];
            }
            (0x8, _, _, 0x1) => {
                // 8xy1 - OR Vx, Vy
                self.v[x] = self.v[x] | self.v[y];
            }
            (0x8, _, _, 0x2) => {
                // 8xy2 - AND Vx, Vy
                self.v[x] = self.v[x] & self.v[y];
            }
            (0x8, _, _, 0x3) => {
                // 8xy3 - XOR Vx, Vy
                self.v[x] = self.v[x] ^ self.v[y];
            }
            (0x8, _, _, 0x4) => {
                // 8xy4 - ADD Vx, Vy
                let x_val = self.v[x] as u16;
                let y_val = self.v[y] as u16;
                let res = x_val + y_val;
                let vf = if res > 255 { 1 } else { 0 };
                self.v[x] = (res & 0xFF) as u8;
                self.v[0xF] = vf;
            }
            (0x8, _, _, 0x5) => {
                // 8xy5 - SUB Vx, Vy
                let x_val = self.v[x] as i16;
                let y_val = self.v[y] as i16;
                let mut res = x_val - y_val;
                let vf = if res < 0 { 0 } else { 1 };
                if res < 0 {
                    res = 256 + res;
                }
                self.v[x] = (res & 0xFF) as u8;
                self.v[0xF] = vf;
            }
            (0x8, _, _, 0x6) => {
                // 8xy6 - SHR Vx {, Vy}
                let vf = self.v[x] & 0x1;
                self.v[x] = self.v[x] >> 1;
                self.v[0xF] = vf;
            }
            (0x8, _, _, 0x7) => {
                // 8xy7 - SUBN Vx, Vy
                let x_val = self.v[x] as i16;
                let y_val = self.v[y] as i16;
                let mut res = y_val - x_val;
                let vf = if res < 0 { 0 } else { 1 };
                if res < 0 {
                    res = 256 + res;
                }
                self.v[x] = (res & 0xFF) as u8;
                self.v[0xF] = vf;
            }
            (0x8, _, _, 0xE) => {
                // 8xyE - SHL Vx {, Vy}
                self.v[0xF] = self.v[x] >> 7 & 0x1;
                self.v[x] = self.v[x] << 1;
            }
            (0x9, ..) => {
                // 9xy0 - SNE Vx, Vy
                if self.v[x] != self.v[y] {
                    self.pc = self.pc + 4;
                    return;
                }
            }
            (0xA, ..) => {
                // Annn - LD I, addr
                self.i = nnn;
            }
            (0xB, ..) => {
                // Bnnn - JP V0, addr
                self.pc = nnn + self.v[0] as u16;
            }
            (0xC, ..) => {
                // Cxkk - RND Vx, byte
                let rnd = rand::random::<u8>();
                self.v[x] = rnd & kk;
            }
            (0xD, ..) => {
                // Dxyn - DRW Vx, Vy, nibble
                let start = self.i as usize;
                let end = start + nibbles[3] as usize;
                let sprites = &self.memory[start..end];
                let collision = self.display.write_sprite(sprites, self.v[x], self.v[y]);
                self.v[0xF] = if collision { 1 } else { 0 };
            }
            (0xE, _, 0x9, 0xE) => {
                // Ex9E - SKP Vx
                if self.keyboard.keys[self.v[x] as usize] == VKey::Down {
                    self.pc = self.pc + 4;
                    return;
                }
            }
            (0xE, _, 0xA, 0x1) => {
                // ExA1 - SKNP Vx
                if self.keyboard.keys[self.v[x] as usize] == VKey::Up {
                    self.pc = self.pc + 4;
                    return;
                }
            }
            (0xF, _, 0x0, 0x7) => {
                // Fx07 - LD Vx, DT
                self.v[x] = self.dt.get_reg();
            }
            (0xF, _, 0x0, 0xA) => {
                // Fx0A - LD Vx, K
                let pressed_key = self.keyboard.keys.iter().position(|&v| v == VKey::Down);
                let mut key_change = false;
                if let Some(pressed_key) = pressed_key {
                    if self.keyboard.prev_keys[pressed_key] == VKey::Up {
                        self.v[x] = pressed_key as u8;
                        key_change = true
                    }
                }
                self.keyboard.prev_keys = self.keyboard.keys;
                if !key_change {
                    return;
                }
            }
            (0xF, _, 0x1, 0x5) => {
                // Fx15 - LD DT, Vx
                self.dt.set_reg(self.v[x]);
            }
            (0xF, _, 0x1, 0x8) => {
                // Fx18 - LD ST, Vx
                self.st.set_reg(self.v[x]);
            }
            (0xF, _, 0x1, 0xE) => {
                // Fx1E - ADD I, Vx
                self.i += self.v[x] as u16;
            }
            (0xF, _, 0x2, 0x9) => {
                // Fx29 - LD F, Vx
                self.i = self.v[x] as u16 * 5;
            }
            (0xF, _, 0x3, 0x3) => {
                // Fx33 - LD B, Vx
                let i = self.i as usize;
                let vx = self.v[x];
                let memslice = &mut self.memory[i..i + 3];
                memslice[0] = vx / 100;
                memslice[1] = (vx / 10) % 10;
                memslice[2] = vx % 10;
            }
            (0xF, _, 0x5, 0x5) => {
                // Fx55 - LD [I], Vx
                let i = self.i as usize;
                let memslice = &mut self.memory[i..i + x + 1];
                memslice.copy_from_slice(&self.v[0..x + 1]);
                //self.i += x as u16 + 1;
            }
            (0xF, _, 0x6, 0x5) => {
                //Fx65 - LD Vx, [I]
                let i = self.i as usize;
                let memslice = &self.memory[i..i + x + 1];
                self.v[0..x + 1].copy_from_slice(memslice);
                //self.i += x as u16 + 1;
            }

            _ => panic!("unknown opcode"),
        }
        self.pc += 2;
    }
}

fn read_memory(mem: &[u8; 4096], addr: u16) -> u16 {
    (mem[addr as usize] as u16) << 8 | mem[addr as usize + 1] as u16
}
