use std::fmt;
use std::time::{Duration, Instant};

use rand::prelude::*;

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
        return if (self._reg_value as u64) < diff {0} else {self._reg_value - diff as u8};
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum VKey {
    Up,
    Down,
}

#[derive(Debug)]
pub struct Keyboard {
    keys: [VKey; 16],
}

impl Default for Keyboard {
    fn default() -> Self {
        Self {
            keys: [VKey::Up; 16],
        }
    }
}

pub struct Display {
    // x: 0 - 63 pixels or 0-7 bytes
    // y: 0 - 31 bytes
    pub cells: [[u8; 8]; 32],
    updates: u64,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            cells: [[0u8; 8]; 32],
            updates: 0,
        }
    }
}

impl Display {
    fn clear(&mut self) {
        for y in 0..32 {
            for x in 0..8 {
                self.cells[y][x] = 0;
            }
        }
        self.updates += 1
    }

    fn render_to_str(&self) -> String {
        let mut string = String::new();
        for y in 0..32 {
            for x in 0..8 {
                for bit in 0..8 {
                    if ((self.cells[y][x] >> (7 - bit)) & 0x1) == 1 {
                        string.push('#');
                        string.push('#');
                    } else {
                        string.push(' ');
                        string.push(' ');
                    }
                }
            }
            string.push('\n');
        }
        string
    }

    fn render_to_buf(&self) -> Vec<u8> {
        let mut buf = Vec::<u8>::new();
        for y in 0..32 {
            for x in 0..8 {
                for bit in 0..8 {
                    if ((self.cells[y][x] >> (7 - bit)) & 0x1) == 1 {
                        buf.push(1);
                    } else {
                        buf.push(0);
                    }
                }
            }
        }
        buf
    }

    fn write_sprite(&mut self, sprite: &[u8], x: u8, y: u8) -> bool {
        let mut collision = false;
        let x = x % 64;
        let y = y % 32;
        for i in 0..sprite.len() {
            let y_roll = ((y as usize + i) % 32) as u8;
            let cur_val = self.get_byte(x, y_roll);
            let new_val = cur_val ^ sprite[i];
            self.set_byte(x, y_roll, new_val);
            if new_val != cur_val {
                collision = true;
            }
        }
        self.updates += 1;
        collision
    }

    fn get_byte(&mut self, x: u8, y: u8) -> u8 {
        let offs_bytes = x as usize / 8;
        let offs_bits = x as usize % 8;
        let line = &self.cells[y as usize];
        let word = (line[offs_bytes] as u16) << 8 | line[(offs_bytes + 1) % 8] as u16;
        let res = ((word >> (8 - offs_bits)) & 0xFF) as u8;
        res
    }

    fn set_byte(&mut self, x: u8, y: u8, val: u8) {
        let offs_bytes = x as usize / 8;
        let offs_bits = x as usize % 8;
        let line = &mut self.cells[y as usize];

        let mut word = (line[offs_bytes] as u16) << 8 | line[(offs_bytes + 1) % 8] as u16;
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
    fn new(code: &[u8], multi: f64) -> Self {
        let mut cpu = Self::default();
        cpu.dt.multi = multi;
        cpu.st.multi = multi;
        cpu.memory[0..80].copy_from_slice(&cpu.display.std_sprites());
        cpu.memory[0x200..0x200 + code.len()].copy_from_slice(code);
        cpu.pc = 0x200;
        cpu
    }

    fn tick(&mut self) -> u16 {
        let instr = read_memory(&self.memory, self.pc);
        //println!("HIER:{:#X}", instr);
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

        match nibbles[0] {
            0 => match kk {
                0xE0 => {
                    self.display.clear();
                }
                0xEE => {
                    self.pc = self.stack[self.sp as usize];
                    self.sp = self.sp - 1;
                }
                _ => panic!("unknown opcode"),
            },
            1 => {
                // JP addr
                self.pc = nnn;
                return;
            }
            2 => {
                // CALL addr
                self.sp = self.sp + 1;
                self.stack[self.sp as usize] = self.pc;
                self.pc = nnn;
                return;
            }
            3 => {
                // SE Vx, byte
                if self.v[x] == kk {
                    self.pc = self.pc + 4;
                    return;
                }
            }
            4 => {
                // SNE Vx, byte
                if self.v[x] != kk {
                    self.pc = self.pc + 4;
                    return;
                }
            }
            5 => {
                // 5xy0 - SE Vx, Vy
                if self.v[x] == self.v[y] {
                    self.pc = self.pc + 4;
                    return;
                }
            }
            6 => {
                // 6xkk - LD Vx, byte
                self.v[x] = kk;
            }
            7 => {
                // 7xkk - ADD Vx, byte
                let tmp = self.v[x] as u16 + kk as u16;
                self.v[x] = (tmp & 0xFF) as u8;
            }
            8 => {
                match nibbles[3] {
                    0 => {
                        // 8xy0 - LD Vx, Vy
                        self.v[x] = self.v[y];
                    }
                    1 => {
                        // 8xy1 - OR Vx, Vy
                        self.v[x] = self.v[x] | self.v[y];
                    }
                    2 => {
                        // 8xy2 - AND Vx, Vy
                        self.v[x] = self.v[x] & self.v[y];
                    }
                    3 => {
                        // 8xy3 - XOR Vx, Vy
                        self.v[x] = self.v[x] ^ self.v[y];
                    }
                    4 => {
                        // 8xy4 - ADD Vx, Vy
                        let x_val = self.v[x] as u16;
                        let y_val = self.v[y] as u16;
                        let res = x_val + y_val;
                        let vf = if res > 255 { 1 } else { 0 };
                        self.v[x] = (res & 0xFF) as u8;
                        self.v[0xF] = vf;
                    }
                    5 => {
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
                    6 => {
                        // 8xy6 - SHR Vx {, Vy}
                        let vf = self.v[x] & 0x1;
                        self.v[x] = self.v[x] >> 1;
                        self.v[0xF] = vf;
                    }
                    7 => {
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
                    0xE => {
                        // 8xyE - SHL Vx {, Vy}
                        self.v[0xF] = self.v[x] >> 7 & 0x1;
                        self.v[x] = self.v[x] << 1;
                    }
                    _ => panic!("unknown opcode"),
                }
            }
            9 => {
                // 9xy0 - SNE Vx, Vy
                if self.v[x] != self.v[y] {
                    self.pc = self.pc + 4;
                    return;
                }
            }
            0xA => {
                // Annn - LD I, addr
                self.i = nnn;
            }
            0xB => {
                // Bnnn - JP V0, addr
                self.pc = nnn + self.v[0] as u16;
            }
            0xC => {
                // Cxkk - RND Vx, byte
                let rnd = rand::random::<u8>();
                self.v[x] = rnd & kk;
            }
            0xD => {
                // Dxyn - DRW Vx, Vy, nibble
                let start = self.i as usize;
                let end = start + nibbles[3] as usize;
                let sprites = &self.memory[start..end];
                let collision = self.display.write_sprite(sprites, self.v[x], self.v[y]);
                self.v[0xF] = if collision { 1 } else { 0 };
            }
            0xE => match kk {
                0x9E => {
                    // Ex9E - SKP Vx
                    if self.keyboard.keys[self.v[x] as usize] == VKey::Down {
                        self.pc = self.pc + 4;
                        return;
                    }
                }
                0xA1 => {
                    // ExA1 - SKNP Vx
                    if self.keyboard.keys[self.v[x] as usize] == VKey::Up {
                        self.pc = self.pc + 4;
                        return;
                    }
                }
                _ => panic!("unknown opcode"),
            },
            0xF => match kk {
                0x07 => {
                    // Fx07 - LD Vx, DT
                    self.v[x] = self.dt.get_reg();
                }
                0x0A => {
                    // Fx0A - LD Vx, K
                    let pressed_key = self
                        .keyboard
                        .keys
                        .iter()
                        .position(|&v| v == VKey::Down);
                    if let Some(pressed_key) = pressed_key {
                        self.v[x] = pressed_key as u8;
                    } else {
                        return;
                    }
                }
                0x15 => {
                    // Fx15 - LD DT, Vx
                    self.dt.set_reg(self.v[x]);
                }
                0x18 => {
                    // Fx18 - LD ST, Vx
                    self.st.set_reg(self.v[x]);
                }
                0x1E => {
                    // Fx1E - ADD I, Vx
                    self.i += self.v[x] as u16;
                }
                0x29 => {
                    // Fx29 - LD F, Vx
                    self.i = self.v[x] as u16 * 5;
                }
                0x33 => {
                    // Fx33 - LD B, Vx
                    let i = self.i as usize;
                    let vx = self.v[x];
                    let memslice = &mut self.memory[i..i + 3];
                    memslice[0] = vx / 100;
                    memslice[1] = (vx / 10) % 10;
                    memslice[2] = vx % 10;
                }
                0x55 => {
                    // Fx55 - LD [I], Vx
                    let i = self.i as usize;
                    let memslice = &mut self.memory[i..i + x + 1];
                    memslice.copy_from_slice(&self.v[0..x + 1]);
                    //self.i += x as u16 + 1;
                }
                0x65 => {
                    //Fx65 - LD Vx, [I]
                    let i = self.i as usize;
                    let memslice = &self.memory[i..i + x + 1];
                    self.v[0..x + 1].copy_from_slice(memslice);
                    //self.i += x as u16 + 1;
                }
                _ => panic!("unknown opcode"),
            },
            _ => panic!("unknown opcode"),
        }
        self.pc += 2;
    }
}

fn read_memory(mem: &[u8; 4096], addr: u16) -> u16 {
    (mem[addr as usize] as u16) << 8 | mem[addr as usize + 1] as u16
}

fn write_memory(mem: &mut [u8; 4096], addr: u16, val: u16) {
    mem[addr as usize] = ((val >> 8) & 0xFF) as u8;
    mem[addr as usize + 1] = (val & 0xFF) as u8;
}

use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::thread;

use minifb::{Key, Scale, Window, WindowOptions};

const WIDTH: usize = 64;
const HEIGHT: usize = 32;

fn main() {
    println!("Hello, world!");
    let path = std::env::args().nth(1).expect("no file given");

    let f = File::open(path).unwrap();
    let mut buf_reader = BufReader::new(f);
    let mut code = Vec::<u8>::new();
    buf_reader.read_to_end(&mut code);

    //let code = include_bytes!("../roms/slipperyslope.ch8");

    let mut cpu = Cpu::new(&code, 1.0);

    let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let mut window_options = WindowOptions::default();
    window_options.scale = Scale::X16;
    let mut window = Window::new("Test - ESC to exit", WIDTH, HEIGHT, window_options)
        .unwrap_or_else(|e| {
            panic!("{}", e);
        });

    //window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));
    window.limit_update_rate(None);

    let mut last_display_update = 0;

    let mut perfcnt_clock_steps = 0u64;
    let mut perfcnt_sprites = 0u64;
    let mut perfcnt = 0u64;
    let mut perfcnt_time = Instant::now();

    let keys = [
        Key::X,
        Key::NumPad1,
        Key::NumPad2,
        Key::NumPad3,
        Key::Q,
        Key::W,
        Key::E,
        Key::A,
        Key::S,
        Key::D,
        Key::Z,
        Key::C,
        Key::NumPad4,
        Key::R,
        Key::F,
        Key::V,
    ];

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if perfcnt % 100 == 0 {
            let delta_t = perfcnt_time.elapsed().as_secs_f64();
            let delta_clock_steps = cpu.clock_steps - perfcnt_clock_steps;
            let delta_sprites = cpu.display.updates - perfcnt_sprites;
            println!(
                "tps: {}; fps: {}",
                delta_clock_steps as f64 / delta_t,
                delta_sprites as f64 / delta_t,
            );
            perfcnt_clock_steps = cpu.clock_steps;
            perfcnt_sprites = cpu.display.updates;
            perfcnt_time = Instant::now();
        }
        perfcnt += 1;

        while cpu.display.updates == last_display_update {
            keys.iter()
                .map(|key| {
                    if window.is_key_down(*key) {
                        VKey::Down
                    } else {
                        VKey::Up
                    }
                })
                .zip(cpu.keyboard.keys.iter_mut())
                .for_each(|(winkey, cpukey)| *cpukey = winkey);
            //println!("{:?}", cpu.keyboard.keys);
            //println!("{:?}", cpu);
            let instr = cpu.tick();
            //thread::sleep(Duration::from_millis(10));
            //println!("Instruction: {:#X}", instr);
            //time::sleep(1);
        }

        if cpu.display.updates != last_display_update {
            last_display_update = cpu.display.updates;
            let display_data = cpu.display.render_to_buf();
            for (disp, b) in display_data.iter().zip(buffer.iter_mut()) {
                //*b = *disp as u32 * 0x00FFFFFF;
                *b = *disp as u32 * 0x00FFAA00 + (1 - *disp) as u32 * 0x00AA4400;
            }
        }

        // We unwrap here as we want this code to exit if it fails. Real applications may want to handle this in a different way
        window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
    }
}
