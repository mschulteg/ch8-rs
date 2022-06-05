use std::sync::mpsc::{self, RecvError, SendError, TryRecvError, TrySendError};
use std::thread;

use super::cpu::{Cpu, VKey, HEIGHT, WIDTH};
use super::perf::PerfLimiter;

use anyhow::Context;
use minifb::{Key, Scale, Window, WindowOptions};

#[derive(Copy, Clone)]
pub struct Emulator {
    pub skip_frames: bool,
    pub fps_limit: Option<f64>,
    pub ips_limit: Option<f64>,
    pub debug: u64,
    pub colors: Option<[u32; 4]>,
}

impl Emulator {
    pub fn new() -> Self {
        Self {
            skip_frames: false,
            fps_limit: None,
            ips_limit: None,
            debug: 0,
            colors: None,
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

    pub fn with_colors(mut self, colors: Option<[u32; 4]>) -> Self {
        self.colors = colors;
        self
    }

    pub fn with_debug(mut self, debug: u64) -> Self {
        self.debug = debug;
        self
    }

    pub fn run(&self, code: Vec<u8>) -> Result<(), anyhow::Error> {
        let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT * 4];
        let mut window_options = WindowOptions::default();
        window_options.scale = Scale::X16;
        window_options.resize = true;
        let mut window = Window::new("CHIP8 - ESC to exit", WIDTH, HEIGHT, window_options)
            .context("Could not create minifb window")?;

        window.limit_update_rate(None);

        let (tx_keys, rx_keys) = mpsc::sync_channel::<[VKey; 16]>(1);
        let (tx_disp, rx_disp) = mpsc::sync_channel::<(Vec<u32>, usize, usize)>(1);
        let (tx_disp_notify, rx_disp_notify) = mpsc::sync_channel::<()>(1);

        let mut perf_io = PerfLimiter::new(self.fps_limit);
        let mut perf_cpu = PerfLimiter::new(self.ips_limit);
        let mut ticker_tps = PerfLimiter::new(Some(1.0));
        let mut ticker_fps = PerfLimiter::new(Some(1.0));
        let debug = self.debug;
        let skip_frames = self.skip_frames;

        let mut cpu = Cpu::new(&code[..], 1.0);
        if let Some(colors) = self.colors {
            cpu.display.colors = colors;
        }

        let cpu_thread = thread::spawn(move || -> Result<(), anyhow::Error> {
            cpu.start_audio()?;
            let reduce_flicker = true;
            loop {
                if debug >= 2 {
                    println!("{:?}", cpu.keyboard.keys);
                    println!("{:?}", cpu);
                    println!("Instruction: {:#X}", cpu.next_instruction());
                }

                // Calculate next instruction
                let instructions_done = cpu.tick()?;

                let send_display = | skip_frames : bool| -> bool {
                    if skip_frames{
                        match tx_disp_notify.try_send(()) {
                            Ok(..) => {
                                match tx_disp.send((
                                    cpu.display.to_buf(),
                                    cpu.display.height,
                                    cpu.display.width,
                                )) {
                                    Ok(..) => {false}
                                    Err(SendError(..)) => {
                                        true
                                    }
                                }
                            }
                            Err(TrySendError::Full(..)) => {false} //skipped frame
                            Err(TrySendError::Disconnected(..)) => true,
                        }
                    } else {
                        // wait until we can send the next frame
                        match tx_disp_notify.send(()) {
                            Ok(..) => {
                                match tx_disp.send((
                                    cpu.display.to_buf(),
                                    cpu.display.height,
                                    cpu.display.width,
                                )) {
                                    Ok(..) => { false}
                                    Err(SendError(..)) => {
                                        true
                                    }
                                }
                            }
                            Err(SendError(..)) => {
                                true
                            }
                        }
                    }
                };

                // If we draw to the real screen only on cpu display state change, we will get a lot of flickering.
                // This is because after most display altering instructions, the frame will be in an
                // incomplete state.
                // We an reduce flickering by updating the display after each cpu instruction regardless of if the
                // cpu display state changed. This time based periodic sampling of the display contents can
                // reduce flicker because statistically most of the time the frame will be in a complete state
                // while the chip 8 rom is waiting for input.
                let always_update = reduce_flicker;
                if always_update || cpu.display.updated {
                    let err = send_display(skip_frames);
                    cpu.display.updated = false;
                    if err {break;}
                }

                match rx_keys.try_recv() {
                    Ok(keys) => {
                        cpu.keyboard.keys = keys;
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => break,
                }
                if instructions_done > 0 {
                    perf_cpu.wait();
                } else {
                    // No instruction was executed, cpu is stuck waiting for key input
                    // Use hard coded delay instead of counting cpu ticks
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                if !ticker_tps.wait_nonblocking() && debug >= 1 {
                    println!("instructions per second (ips): {}", perf_cpu.get_fps());
                }
            }
            Ok(())
        });

        while window.is_open() && !window.is_key_down(Key::Escape) {
            let cpu_keys = convert_keys(&window);
            match tx_keys.try_send(cpu_keys) {
                Ok(..) => {}
                Err(TrySendError::Full(..)) => {} //skipped input
                Err(TrySendError::Disconnected(..)) => break,
            }

            match rx_disp_notify.try_recv() {
                Ok(..) => match rx_disp.recv() {
                    Ok((display_buf, height, width)) => {
                        buffer[..height * width].copy_from_slice(&display_buf[..]);
                        window
                            .update_with_buffer(&buffer, width, height)
                            .context("Updating minifb display buffer failed")?;
                    }
                    Err(RecvError) => break,
                },
                Err(TryRecvError::Empty) => {
                    window.update();
                }
                Err(TryRecvError::Disconnected) => break,
            }
            perf_io.wait();
            if !ticker_fps.wait_nonblocking() && debug >= 1 {
                println!("frames per second       (fps): {}", perf_io.get_fps());
            }
        }
        println!("Exiting");
        drop(rx_disp);
        drop(tx_keys);
        cpu_thread.join().unwrap().context("Failed in CPU thread")?;
        Ok(())
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
