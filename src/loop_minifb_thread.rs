use std::fs::File;
use std::io::BufReader;
use std::io::Read;

use std::sync::mpsc::{self, TryRecvError, TrySendError};
use std::thread;

use super::cpu::{display_cells_to_buf, Cpu, VKey};
use super::perf::PerfLimiter;

use minifb::{Key, Scale, Window, WindowOptions};

struct Emulator {
    pub skip_frames: bool,
    pub fps_limit: Option<f64>,
    pub ips_limit: Option<f64>,
    pub debug: bool,
}

impl Emulator {
    pub fn new() -> Self {
        Self {
            skip_frames: true,
            fps_limit: None,
            ips_limit: None,
            debug: false,
        }
    }

    pub fn with_skip_frames(mut self, skip: bool) -> Self {
        self.skip_frames = skip;
        self
    }

    pub fn with_fps_limit(mut self, limit: f64) -> Self {
        self.fps_limit = Some(limit);
        self
    }

    pub fn with_ips_limit(mut self, limit: f64) -> Self {
        self.ips_limit = Some(limit);
        self
    }

    pub fn with_debug(mut self) -> Self {
        self.debug = true;
        self
    }

    pub fn run(&self, code: &[u8]) {
        let mut cpu = Cpu::new(code, 1.0);

        let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];
        let mut window_options = WindowOptions::default();
        window_options.scale = Scale::X16;
        let mut window = Window::new("Test - ESC to exit", WIDTH, HEIGHT, window_options)
            .unwrap_or_else(|e| {
                panic!("{}", e);
            });

        window.limit_update_rate(None);

        let (tx_keys, rx_keys) = mpsc::sync_channel::<[VKey; 16]>(1);
        let (tx_disp, rx_disp) = mpsc::sync_channel::<[[u8; 8]; 32]>(1);

        let mut perf_io = PerfLimiter::new(self.fps_limit);
        let mut perf_cpu = PerfLimiter::new(self.ips_limit);
        let mut ticker_tps = PerfLimiter::new(Some(1.0));
        let mut ticker_fps = PerfLimiter::new(Some(1.0));
        let debug = self.debug;
        let cpu_thread = thread::spawn(move || loop {
            if debug {
                println!("{:?}", cpu.keyboard.keys);
                println!("{:?}", cpu);
                println!("Instruction: {:#X}", cpu.next_instruction());
            }

            cpu.tick();

            //this variant skips frames
            match tx_disp.try_send(cpu.display.cells) {
                Ok(..) => {}
                Err(TrySendError::Full(..)) => {} //skipped frame
                Err(TrySendError::Disconnected(..)) => break,
            }
            // if cpu.display.updated {
            //     cpu.display.updated = false;
            //     match tx_disp.send(cpu.display.cells) {
            //         Ok(..) => {},
            //         Err(SendError(..)) => {break},
            //     }
            // }

            match rx_keys.try_recv() {
                Ok(keys) => {
                    cpu.keyboard.keys = keys;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => break,
            }
            perf_cpu.wait();
            if !ticker_tps.wait_nonblocking() {
                println!("tps: {}", perf_cpu.get_fps());
            }
        });

        while window.is_open() && !window.is_key_down(Key::Escape) {
            let cpu_keys = set_keys(&window);
            //tx_keys.send(cpu_keys).unwrap();
            match tx_keys.try_send(cpu_keys) {
                Ok(..) => {}
                Err(TrySendError::Full(..)) => {} //skipped input
                Err(TrySendError::Disconnected(..)) => break,
            }

            match rx_disp.try_recv() {
                Ok(display_cells) => {
                    let display_data = display_cells_to_buf(display_cells);
                    for (disp, b) in display_data.iter().zip(buffer.iter_mut()) {
                        *b = *disp as u32 * 0x00FFAA00 + (1 - *disp) as u32 * 0x00AA4400;
                    }
                    window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
                }
                Err(TryRecvError::Empty) => {
                    window.update();
                }
                Err(TryRecvError::Disconnected) => break,
            }
            perf_io.wait();
            if !ticker_fps.wait_nonblocking() {
                println!("tps: {}", perf_io.get_fps());
            }
        }
        println!("exiting");
        drop(rx_disp);
        drop(tx_keys);
        cpu_thread.join().unwrap();
    }
}

const WIDTH: usize = 64;
const HEIGHT: usize = 32;

fn set_keys(window: &Window) -> [VKey; 16] {
    let keys = [
        Key::X,
        Key::Key1,
        Key::Key2,
        Key::Key3,
        Key::Q,
        Key::W,
        Key::E,
        Key::A,
        Key::S,
        Key::D,
        Key::Z,
        Key::C,
        Key::Key4,
        Key::R,
        Key::F,
        Key::V,
    ];
    let mut cpu_keys = [VKey::Up; 16];
    keys.iter()
        .map(|key| {
            if window.is_key_down(*key) {
                VKey::Down
            } else {
                VKey::Up
            }
        })
        .zip(cpu_keys.iter_mut())
        .for_each(|(winkey, cpukey)| *cpukey = winkey);
    cpu_keys
}

pub fn event_loop() {
    let path = std::env::args().nth(1).expect("No file given");

    let f = File::open(path).unwrap();
    let mut buf_reader = BufReader::new(f);
    let mut code = Vec::<u8>::new();
    buf_reader
        .read_to_end(&mut code)
        .expect("Could not read file to end");

    let emulator = Emulator::new().with_fps_limit(60.0).with_ips_limit(10000.0);
    emulator.run(&code[..]);
}
