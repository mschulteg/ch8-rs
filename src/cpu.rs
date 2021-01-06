use std::convert::TryInto;
use std::fmt;
use std::time::{Instant, Duration};

use super::sound::Sound;

pub const WIDTH: usize = 64;
pub const HEIGHT: usize = 32;
pub const MEMSIZE: usize = 65536;

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

    fn time_left(&self) -> Option<Duration> {
        if self._reg_value == 0 {
            return None;
        }
        Some(Duration::from_secs_f64(self._reg_value as f64 / self.freq_hz))
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

pub struct Plane {
    // x: 0 - 63 (or 127) pixels are stored in 0-7 (or 15) bytes
    // y: 0 - 31 (or 63) bytes
    cells: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

impl Plane {
    fn new(height: usize, width: usize) -> Self {
        Plane {
            cells: vec![0u8; height * width],
            width,
            height,
        }
    }

    fn scroll_down(&mut self, n: u8) {
        self.cells.rotate_right(n as usize * self.width / 8);
        for pixel_byte in self.cells[0..n as usize * self.width / 8].iter_mut() {
            *pixel_byte = 0;
        }
    }

    fn scroll_up(&mut self, n: u8) {
        for pixel_byte in self.cells[0..n as usize * self.width / 8].iter_mut() {
            *pixel_byte = 0;
        }
        self.cells.rotate_left(n as usize * self.width / 8);
    }

    fn scroll_right(&mut self) {
        let mut last_nibble = 0u8;
        for val in self.cells.iter_mut() {
            let tmp = *val & 0xF;
            *val = (*val >> 4) | (last_nibble << 4);
            last_nibble = tmp;
        }
    }

    fn scroll_left(&mut self) {
        let mut last_nibble = 0u8;
        for val in self.cells.iter_mut().rev() {
            let tmp = (*val & 0xF0) >> 4;
            *val = (*val << 4) | last_nibble;
            last_nibble = tmp;
        }
    }

    fn clear(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width / 8 {
                self.cells[y * (self.width / 8) + x] = 0;
            }
        }
    }

    fn write_sprite(&mut self, sprite: &[u8], x: u8, y: u8) -> bool {
        let mut collision = false;
        let x = x % self.width as u8;
        let y = y % self.height as u8;
        for i in 0..sprite.len() {
            let y_roll = ((y as usize + i) % self.height) as u8;
            let cur_val = self.get_byte(x, y_roll);
            let new_val = cur_val ^ sprite[i];
            let cleared = cur_val & sprite[i];
            self.set_byte(x, y_roll, new_val);
            if cleared != 0 {
                collision = true;
            }
        }
        collision
    }

    fn write_sprite16(&mut self, sprite: &[u8; 32], x: u8, y: u8) -> bool {
        let mut collision = false;
        let mut left = [0u8; 16];
        let mut right = [0u8; 16];
        for (src, dest) in sprite.iter().step_by(2).zip(left.iter_mut()) {
            *dest = *src;
        }
        for (src, dest) in sprite.iter().skip(1).step_by(2).zip(right.iter_mut()) {
            *dest = *src;
        }
        collision |= self.write_sprite(&left[..], x, y);
        collision |= self.write_sprite(&right[..], x + 8, y);
        collision
    }

    fn get_byte(&mut self, x: u8, y: u8) -> u8 {
        let offs_bytes = x as usize / 8;
        let offs_bits = x as usize % 8;
        let line_offs = y as usize * self.width / 8;

        let word = (self.cells[line_offs + offs_bytes] as u16) << 8
            | self.cells[line_offs + (offs_bytes + 1) % (self.width / 8)] as u16;
        let res = ((word >> (8 - offs_bits)) & 0xFF) as u8;
        res
    }

    fn set_byte(&mut self, x: u8, y: u8, val: u8) {
        let offs_bytes = x as usize / 8;
        let offs_bits = x as usize % 8;
        let line_offs = y as usize * self.width / 8;

        let mut word = (self.cells[line_offs + offs_bytes] as u16) << 8
            | self.cells[line_offs + (offs_bytes + 1) % (self.width / 8)] as u16;
        word &= !(0xFF << (8 - offs_bits));
        word |= (val as u16) << (8 - offs_bits);
        self.cells[line_offs + offs_bytes] = ((word >> 8) & 0xFF) as u8;
        self.cells[line_offs + (offs_bytes + 1) % (self.width / 8)] = (word & 0xFF) as u8;
    }
}

pub struct Display {
    pub planes: Vec<Plane>,
    pub width: usize,
    pub height: usize,
    pub updates: u64,
    pub updated: bool,
    pub extended: bool,
    pub colors: [u32; 4],
    pub active_planes: u8,
}

impl Display {
    fn new(height: usize, width: usize) -> Self {
        Self {
            planes: vec![Plane::new(height, width), Plane::new(height, width)],
            width: width,
            height: height,
            updates: 0,
            updated: true,
            extended: false,
            colors: [0x00AA4400, 0x00FFAA00, 0x00AAAAAA, 0x00000000],
            active_planes: 0x1,
        }
    }

    fn set_extended(&mut self, ext: bool) {
        if ext {
            self.height = HEIGHT * 2;
            self.width = WIDTH * 2;
        } else {
            self.height = HEIGHT;
            self.width = WIDTH;
        }
        self.extended = ext;
        self.planes = vec![
            Plane::new(self.height, self.width),
            Plane::new(self.height, self.width),
        ];
    }

    fn flag_updated(&mut self) {
        self.updated = true;
        self.updates += 1;
    }

    pub fn to_buf(&self) -> Vec<u32> {
        let cells1 = &self.planes[0].cells;
        let cells2 = &self.planes[1].cells;
        let mut buf = Vec::<u32>::with_capacity(self.height * self.width);
        for y in 0..self.height {
            for x in 0..self.width / 8 {
                for bit in 0..8 {
                    let mut bitplane = 0;
                    bitplane |= ((cells1[y * (self.width / 8) + x] >> (7 - bit)) & 0x1) << 0;
                    bitplane |= ((cells2[y * (self.width / 8) + x] >> (7 - bit)) & 0x1) << 1;
                    buf.push(self.colors[bitplane as usize]);
                }
            }
        }
        buf
    }

    fn scroll_down(&mut self, n: u8) {
        for (i, plane) in self.planes.iter_mut().enumerate() {
            if (self.active_planes >> i as u8) & 0x1 == 1 {
                plane.scroll_down(n);
            }
        }
        self.flag_updated();
    }

    fn scroll_up(&mut self, n: u8) {
        for (i, plane) in self.planes.iter_mut().enumerate() {
            if (self.active_planes >> i as u8) & 0x1 == 1 {
                plane.scroll_up(n);
            }
        }
        self.flag_updated();
    }

    fn scroll_right(&mut self) {
        for (i, plane) in self.planes.iter_mut().enumerate() {
            if (self.active_planes >> i as u8) & 0x1 == 1 {
                plane.scroll_right();
            }
        }
        self.flag_updated();
    }

    fn scroll_left(&mut self) {
        for (i, plane) in self.planes.iter_mut().enumerate() {
            if (self.active_planes >> i as u8) & 0x1 == 1 {
                plane.scroll_left();
            }
        }
        self.flag_updated();
    }

    fn clear(&mut self) {
        for (i, plane) in self.planes.iter_mut().enumerate() {
            if (self.active_planes >> i as u8) & 0x1 == 1 {
                plane.clear();
            }
        }
        self.flag_updated();
    }

    fn write_sprite(&mut self, sprite: &[u8], x: u8, y: u8) -> bool {
        let mut collision = false;
        match self.active_planes {
            0x3 => {
                let length = sprite.len();
                collision |= self.planes[0].write_sprite(&sprite[..length / 2], x, y);
                collision |= self.planes[1].write_sprite(&sprite[length / 2..], x, y);
            }
            _ => {
                for (i, plane) in self.planes.iter_mut().enumerate() {
                    if (self.active_planes >> i as u8) & 0x1 == 1 {
                        collision |= plane.write_sprite(sprite, x, y);
                    }
                }
            }
        }
        self.flag_updated();
        collision
    }

    fn write_sprite16(&mut self, sprite: &[u8], x: u8, y: u8) -> Result<bool, anyhow::Error> {
        let mut collision = false;
        match self.active_planes {
            0x3 => {
                let length = sprite.len();
                collision |=
                    self.planes[0].write_sprite16(&sprite[..length / 2].try_into()?, x, y);
                collision |=
                    self.planes[1].write_sprite16(&sprite[length / 2..].try_into()?, x, y);
            }
            _ => {
                for (i, plane) in self.planes.iter_mut().enumerate() {
                    if (self.active_planes >> i as u8) & 0x1 == 1 {
                        collision |= plane.write_sprite16(sprite.try_into().unwrap(), x, y);
                    }
                }
            }
        }
        self.flag_updated();
        Ok(collision)
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

    fn hires_sprites(&self) -> [u8; 100] {
        [
            0x3C, 0x7E, 0xE7, 0xC3, 0xC3, 0xC3, 0xC3, 0xE7, 0x7E, 0x3C, 0x18, 0x38, 0x58, 0x18,
            0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x3E, 0x7F, 0xC3, 0x06, 0x0C, 0x18, 0x30, 0x60,
            0xFF, 0xFF, 0x3C, 0x7E, 0xC3, 0x03, 0x0E, 0x0E, 0x03, 0xC3, 0x7E, 0x3C, 0x06, 0x0E,
            0x1E, 0x36, 0x66, 0xC6, 0xFF, 0xFF, 0x06, 0x06, 0xFF, 0xFF, 0xC0, 0xC0, 0xFC, 0xFE,
            0x03, 0xC3, 0x7E, 0x3C, 0x3E, 0x7C, 0xC0, 0xC0, 0xFC, 0xFE, 0xC3, 0xC3, 0x7E, 0x3C,
            0xFF, 0xFF, 0x03, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x60, 0x60, 0x3C, 0x7E, 0xC3, 0xC3,
            0x7E, 0x7E, 0xC3, 0xC3, 0x7E, 0x3C, 0x3C, 0x7E, 0xC3, 0xC3, 0x7F, 0x3F, 0x03, 0x03,
            0x3E, 0x7C,
        ]
    }
}

pub struct Cpu {
    pub display: Display,
    pub keyboard: Keyboard,
    pub sound: Sound,
    pub sound_memory: [u8; 16],
    pub dt: Timer,
    pub st: Timer,
    pub memory: [u8; MEMSIZE],
    pub v: [u8; 16],
    pub pc: u16,
    pub sp: u8,
    pub stack: [u16; 16],
    pub i: u16,
    pub clock_steps: u64,
    pub repl: [u8; 8],
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            display: Display::new(HEIGHT, WIDTH),
            keyboard: Keyboard::default(),
            sound: Sound::new(4000.0),
            sound_memory: [0xAAu8; 16],
            dt: Timer::new(),
            st: Timer::new(),
            memory: [0u8; MEMSIZE],
            v: [0u8; 16],
            pc: 0x200,
            sp: 0,
            stack: [0u16; 16],
            i: 0,
            clock_steps: 0,
            repl: [0u8; 8],
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
        cpu.memory[80..180].copy_from_slice(&cpu.display.hires_sprites());
        cpu.memory[0x200..0x200 + code.len()].copy_from_slice(code);
        cpu.pc = 0x200;
        cpu
    }

    pub fn start_audio(&mut self) -> Result<(), anyhow::Error> {
        self.sound.start()?;
        Ok(())
    }

    pub fn next_instruction(&self) -> u16 {
        read_memory(&self.memory, self.pc)
    }

    pub fn skip_instruction(&mut self) {
        self.pc += 2;
        if self.next_instruction() == 0xF000 {
            self.pc += 2;
        }
    }

    pub fn tick(&mut self) -> Result<u16, anyhow::Error> {
        let instr = self.next_instruction();
        self.process_instruction(instr)?;
        self.clock_steps += 1;
        //assert!((self.pc % 2) == 0, "program counter is not even");
        // slipperyslope jumps to uneven instruction (level-unpack at 0x265 (0x65 in file))
        Ok(instr)
    }

    fn process_instruction(&mut self, instr: u16) -> Result<(), anyhow::Error>{
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
            (0x0, 0x0, 0xC, _) => {
                // 0x00CN Scroll display N lines down
                self.display.scroll_down(nibbles[3]);
            }
            (0x0, 0x0, 0xD, _) => {
                // 0x00DN - Scroll display N lines up
                self.display.scroll_up(nibbles[3]);
            }
            (0x0, _, 0xE, 0x0) => {
                self.display.clear();
            }
            (0x0, _, 0xE, 0xE) => {
                self.pc = self.stack[self.sp as usize];
                self.sp = self.sp - 1;
            }
            (0x0, 0x0, 0xF, 0xB) => {
                // Scroll display 4 pixels right
                self.display.scroll_right();
            }
            (0x0, 0x0, 0xF, 0xC) => {
                // Scroll display 4 pixels left
                self.display.scroll_left();
            }
            (0x0, 0x0, 0xF, 0xD) => {
                // Exit CHIP interpreter
                println!("TODO: EXIT");
            }
            (0x0, 0x0, 0xF, 0xE) => {
                // Disable extended screen mode
                self.display.set_extended(false);
            }
            (0x0, 0x0, 0xF, 0xF) => {
                // Enable extended screen mode
                self.display.set_extended(true);
            }
            (0x1, ..) => {
                // JP addr
                self.pc = nnn;
                return Ok(());
            }
            (0x2, ..) => {
                // CALL addr
                self.sp = self.sp + 1;
                self.stack[self.sp as usize] = self.pc;
                self.pc = nnn;
                return Ok(());
            }
            (0x3, ..) => {
                // SE Vx, byte
                if self.v[x] == kk {
                    self.skip_instruction();
                }
            }
            (0x4, ..) => {
                // SNE Vx, byte
                if self.v[x] != kk {
                    self.skip_instruction();
                }
            }
            (0x5, _, _, 0) => {
                // 5xy0 - SE Vx, Vy
                if self.v[x] == self.v[y] {
                    self.skip_instruction();
                }
            }
            (0x5, _, _, 2) => {
                // 5xy2 - LD [I], Vx-Vy
                let i = self.i as usize;
                let range = y - x;
                let memslice = &mut self.memory[i..i + range + 1];
                memslice.copy_from_slice(&self.v[x..y + 1]);
                //self.i += x as u16 + 1;
            }
            (0x5, _, _, 3) => {
                // 5xy3 - LD Vx-Vy, [I]
                let i = self.i as usize;
                let range = y - x;
                let memslice = &self.memory[i..i + range + 1];
                self.v[x..y + 1].copy_from_slice(memslice);
                //self.i += x as u16 + 1;
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
                // quirk - original
                //let vf = self.v[x] & 0x1;
                //self.v[x] = self.v[x] >> 1;
                let vf = self.v[y] & 0x1;
                self.v[x] = self.v[y] >> 1;
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

                // quirk - original
                //self.v[0xF] = self.v[x] >> 7 & 0x1;
                //self.v[x] = self.v[x] << 1;
                self.v[0xF] = self.v[y] >> 7 & 0x1;
                self.v[x] = self.v[y] << 1;
            }
            (0x9, ..) => {
                // 9xy0 - SNE Vx, Vy
                if self.v[x] != self.v[y] {
                    self.skip_instruction();
                }
            }
            (0xA, ..) => {
                // Annn - LD I, addr
                self.i = nnn;
            }
            (0xB, ..) => {
                // Bnnn - JP V0, addr
                //self.pc = nnn + self.v[x] as u16;
                self.pc = nnn + self.v[0] as u16;
                return Ok(());
            }
            (0xC, ..) => {
                // Cxkk - RND Vx, byte
                let rnd = rand::random::<u8>();
                self.v[x] = rnd & kk;
            }
            (0xD, ..) => {
                // Dxyn - DRW Vx, Vy, nibble
                let start = self.i as usize;
                if nibbles[3] == 0 {
                    let end = if self.display.active_planes == 0x3 {
                        start + 64 as usize
                    } else {
                        start + 32 as usize
                    };
                    let sprites = &self.memory[start..end];
                    let collision = self.display.write_sprite16(sprites, self.v[x], self.v[y])?;
                    self.v[0xF] = if collision { 1 } else { 0 };
                } else {
                    let end = if self.display.active_planes == 0x3 {
                        start + nibbles[3] as usize * 2
                    } else {
                        start + nibbles[3] as usize
                    };

                    let sprites = &self.memory[start..end];
                    let collision = self.display.write_sprite(sprites, self.v[x], self.v[y]);
                    self.v[0xF] = if collision { 1 } else { 0 };
                }
            }
            (0xE, _, 0x9, 0xE) => {
                // Ex9E - SKP Vx
                if self.keyboard.keys[self.v[x] as usize] == VKey::Down {
                    self.skip_instruction();
                }
            }
            (0xE, _, 0xA, 0x1) => {
                // ExA1 - SKNP Vx
                if self.keyboard.keys[self.v[x] as usize] == VKey::Up {
                    self.skip_instruction();
                }
            }
            (0xF, 0x0, 0x0, 0x0) => {
                // F000 NNNN - load NNNN to i
                self.pc += 2;
                self.i = read_memory(&self.memory, self.pc);
            }
            (0xF, _, 0x0, 0x1) => {
                // 0xFN01 Select drawing plane by bitmask (0 <= n <= 3)
                self.display.active_planes = nibbles[1];
            }
            (0xF, 0x0, 0x0, 0x2) => {
                // 0xF002 - Store 16 bytes starting at i in the audio pattern buffer.
                let i = self.i as usize;
                let samples = &self.memory[i..i+16];
                self.sound_memory.copy_from_slice(samples);
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
                    return Ok(());
                }
            }
            (0xF, _, 0x1, 0x5) => {
                // Fx15 - LD DT, Vx
                self.dt.set_reg(self.v[x]);
            }
            (0xF, _, 0x1, 0x8) => {
                // Fx18 - LD ST, Vx
                self.st.set_reg(self.v[x]);
                if let Some(duration) = self.st.time_left() {
                    self.sound.play_samples_1bit(&self.sound_memory[..], duration);
                }
            }
            (0xF, _, 0x1, 0xE) => {
                // Fx1E - ADD I, Vx
                self.i += self.v[x] as u16;
            }
            (0xF, _, 0x2, 0x9) => {
                // Fx29 - LD F, Vx
                self.i = self.v[x] as u16 * 5;
            }
            (0xF, _, 0x3, 0x0) => {
                // Fx30 - LD (Hires)F, Vx
                self.i = self.v[x] as u16 * 10 + 16 * 5;
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
            (0xF, _, 0x7, 0x5) => {
                // Fx75 - LD repl, Vx
                let repl_slice = &mut self.repl[0..x + 1];
                repl_slice.copy_from_slice(&self.v[0..x + 1]);
            }
            (0xF, _, 0x8, 0x5) => {
                // Fx85 - LD Vx, repl
                let memslice = &self.memory[0..x + 1];
                self.v[0..x + 1].copy_from_slice(memslice);
            }

            _ => panic!("unknown opcode: {}", instr),
        }
        self.pc += 2;
        Ok(())
    }
}

fn read_memory(mem: &[u8; MEMSIZE], addr: u16) -> u16 {
    (mem[addr as usize] as u16) << 8 | mem[addr as usize + 1] as u16
}
