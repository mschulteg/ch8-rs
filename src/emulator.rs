use std::sync::mpsc::{self, SendError, TryRecvError, TrySendError, RecvError};
use std::thread;

use super::cpu::{Cpu, VKey, WIDTH, HEIGHT};
use super::perf::PerfLimiter;

use minifb::{Key, Scale, Window, WindowOptions};



#[derive(Copy, Clone)]
pub struct Emulator {
    pub skip_frames: bool,
    pub fps_limit: Option<f64>,
    pub ips_limit: Option<f64>,
    pub debug: u64,
}

impl Emulator {
    pub fn new() -> Self {
        Self {
            skip_frames: false,
            fps_limit: None,
            ips_limit: None,
            debug: 0,
        }
    }

    pub fn with_skip_frames(mut self, skip: bool) -> Self {
        self.skip_frames = skip;
        self
    }

    pub fn with_fps_limit(mut self, limit: Option<f64>) -> Self {
        self.fps_limit = limit;
        self
    }

    pub fn with_ips_limit(mut self, limit: Option<f64>) -> Self {
        self.ips_limit = limit;
        self
    }

    pub fn with_debug(mut self, debug: u64) -> Self {
        self.debug = debug;
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
        let (tx_disp, rx_disp) = mpsc::sync_channel::<(Vec<u8>, usize, usize)>(1);
        let (tx_disp_notify, rx_disp_notify) = mpsc::sync_channel::<()>(1);

        let mut perf_io = PerfLimiter::new(self.fps_limit);
        let mut perf_cpu = PerfLimiter::new(self.ips_limit);
        let mut ticker_tps = PerfLimiter::new(Some(1.0));
        let mut ticker_fps = PerfLimiter::new(Some(1.0));
        let debug = self.debug;
        let skip_frames = self.skip_frames;
        let cpu_thread = thread::spawn(move || loop {
            if debug >= 2 {
                println!("{:?}", cpu.keyboard.keys);
                println!("{:?}", cpu);
                println!("Instruction: {:#X}", cpu.next_instruction());
            }

            cpu.tick();

            //this variant skips frames
            if skip_frames {
                match tx_disp_notify.try_send(()) {
                    Ok(..) => {
                        match tx_disp.send((cpu.display.to_buf(), cpu.display.height, cpu.display.width)) {
                            Ok(..) => {}
                            Err(SendError(..)) => {break;}
                        }
                    }
                    Err(TrySendError::Full(..)) => {} //skipped frame
                    Err(TrySendError::Disconnected(..)) => break,
                }
            } else {
                if cpu.display.updated {
                    cpu.display.updated = false;
                    match tx_disp.send((cpu.display.cells.clone(), cpu.display.height, cpu.display.width)) {
                        Ok(..) => {}
                        Err(SendError(..)) => break,
                    }
                }
            }

            match rx_keys.try_recv() {
                Ok(keys) => {
                    cpu.keyboard.keys = keys;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => break,
            }
            perf_cpu.wait();
            if !ticker_tps.wait_nonblocking() && debug >= 1{
                println!("tps: {}", perf_cpu.get_fps());
            }
        });

        while window.is_open() && !window.is_key_down(Key::Escape) {
            let cpu_keys = convert_keys(&window);
            match tx_keys.try_send(cpu_keys) {
                Ok(..) => {}
                Err(TrySendError::Full(..)) => {} //skipped input
                Err(TrySendError::Disconnected(..)) => break,
            }

            match rx_disp_notify.try_recv() {
                Ok(..) => {
                    match rx_disp.recv() {
                        Ok((display_buf, height, width)) => {
                                for (disp, b) in display_buf.iter().zip(buffer.iter_mut()) {
                                    *b = *disp as u32 * 0x00FFAA00 + (1 - *disp) as u32 * 0x00AA4400;
                                }
                                window.update_with_buffer(&buffer, width, height).unwrap();
                            }
                        Err(RecvError) => break,
                    }
                }
                Err(TryRecvError::Empty) => {
                    window.update();
                }
                Err(TryRecvError::Disconnected) => break,
            }
            perf_io.wait();
            if !ticker_fps.wait_nonblocking() && debug >= 1{
                println!("fps: {}", perf_io.get_fps());
            }
        }
        println!("exiting");
        drop(rx_disp);
        drop(tx_keys);
        cpu_thread.join().unwrap();
    }
}

fn convert_keys(window: &Window) -> [VKey; 16] {
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
