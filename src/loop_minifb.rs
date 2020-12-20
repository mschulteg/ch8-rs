use std::fs::File;
use std::io::BufReader;
use std::io::Read;

use super::cpu::{Cpu, Keyboard, VKey};
use super::perf::PerfLimiter;

use minifb::{Key, Scale, Window, WindowOptions};


const WIDTH: usize = 64;
const HEIGHT: usize = 32;

fn set_keys(window: &Window, cpu_keyboard: &mut Keyboard) {
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
    keys.iter()
        .map(|key| {
            if window.is_key_down(*key) {
                VKey::Down
            } else {
                VKey::Up
            }
        })
        .zip(cpu_keyboard.keys.iter_mut())
        .for_each(|(winkey, cpukey)| *cpukey = winkey);
}

pub fn event_loop() {
    let path = std::env::args().nth(1).expect("No file given");

    let f = File::open(path).unwrap();
    let mut buf_reader = BufReader::new(f);
    let mut code = Vec::<u8>::new();
    buf_reader.read_to_end(&mut code).expect("Could not read file to end");

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

    let mut perf_cycles = PerfLimiter::new(100000.0);
    let mut perf_display = PerfLimiter::new(100.0);
    println!("{}", perf_display.every_nths);
    while window.is_open() && !window.is_key_down(Key::Escape) {
        loop {
            set_keys(&window, &mut cpu.keyboard);
            // println!("{:?}", cpu.keyboard.keys);
            // println!("{:?}", cpu);
            // println!("Instruction: {:#X}", read_memory(&cpu.memory, cpu.pc));
            cpu.tick();
            if !perf_display.wait_nonblocking() {
                break;
            }
            perf_cycles.wait();
        }

        println!("tps: {}; fps: {}", perf_cycles.get_fps(), perf_display.get_fps());

        if cpu.display.updated {
            cpu.display.updated = false;
            let display_data = cpu.display.render_to_buf();
            for (disp, b) in display_data.iter().zip(buffer.iter_mut()) {
                //*b = *disp as u32 * 0x00FFFFFF;
                *b = *disp as u32 * 0x00FFAA00 + (1 - *disp) as u32 * 0x00AA4400;
            }
            window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
        }
        else {
            window.update();
        }
    }
}
