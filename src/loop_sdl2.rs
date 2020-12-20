use std::fs::File;
use std::io::BufReader;
use std::io::Read;

use std::time::{Duration, Instant};


use super::cpu::{Cpu, Keyboard, VKey};
use super::perf::PerfLimiter;


use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::gfx::primitives::DrawRenderer;



const WIDTH: u32 = 64;
const HEIGHT: u32 = 32;
const SCALE: u32 = 16;

fn set_keys(keycode: Option<Keycode>, cpu_keyboard: &mut Keyboard) {
    let keys = [
        Keycode::X,
        Keycode::Num1,
        Keycode::Num2,
        Keycode::Num3,
        Keycode::Q,
        Keycode::W,
        Keycode::E,
        Keycode::A,
        Keycode::S,
        Keycode::D,
        Keycode::Z,
        Keycode::C,
        Keycode::Num4,
        Keycode::R,
        Keycode::F,
        Keycode::V,
    ];
    if let Some(keycode) = keycode {
        keys.iter()
            .map(|key| {
                if keycode == *key {
                    VKey::Down
                } else {
                    VKey::Up
                }
            })
            .zip(cpu_keyboard.keys.iter_mut())
            .for_each(|(winkey, cpukey)| *cpukey = winkey);
    }else {
        cpu_keyboard.keys = [VKey::Up; 16];
    }
}

pub fn event_loop() {
    let path = std::env::args().nth(1).expect("No file given");

    let f = File::open(path).unwrap();
    let mut buf_reader = BufReader::new(f);
    let mut code = Vec::<u8>::new();
    buf_reader.read_to_end(&mut code).expect("Could not read file to end");

    let mut cpu = Cpu::new(&code, 1.0);

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
 
    let window = video_subsystem.window("rust-sdl2 demo", WIDTH * SCALE, HEIGHT * SCALE)
        .position_centered()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_draw_color(Color::RGB(0, 255, 255));
    canvas.set_scale(SCALE as f32, SCALE as f32);
    canvas.clear();
    canvas.present();

    let mut perf_cycles = PerfLimiter::new(0.0);
    let mut perf_cnt = 0u64;

    let mut event_pump = sdl_context.event_pump().unwrap();
    'running: loop {
        //canvas.set_draw_color(Color::RGB(i, 64, 255 - i));
        //canvas.clear();
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running
                },
                Event::KeyDown{keycode: code, ..} => {set_keys(code, &mut cpu.keyboard)}
                Event::KeyUp{keycode: code, ..} => {set_keys(None, &mut cpu.keyboard)}
                _ => {}
            }
        }
        // The rest of the game loop goes here...


        cpu.tick();
        if cpu.display.updated {
            canvas.clear();
            cpu.display.updated = false;
            let display_data = cpu.display.render_to_buf();
            for (i, pix) in display_data.iter().enumerate() {
                let value = *pix as u32 * 0xFFFFAA00 + (1 - *pix) as u32 * 0xFFAA4400;
                canvas.pixel((i % WIDTH as usize) as i16, (i / WIDTH as usize) as i16, value).unwrap();
            }
            canvas.present();
        }
        perf_cycles.wait();
        if perf_cnt == 100 {
            perf_cnt = 0;
            println!("{}", perf_cycles.get_fps());
        }
        perf_cnt += 1;
        //::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
