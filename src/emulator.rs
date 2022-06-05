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

        // Main loop
        let upscaling = 4;
        let conf = miniquad::conf::Conf {
            window_title: "Miniquad".to_string(),
            window_width: WIDTH as i32 * 4 * upscaling,
            window_height: HEIGHT as i32 * 4 * upscaling,
            fullscreen: false,
            ..Default::default()
        };
        miniquad::start(conf, |mut ctx| {
            Box::new(Stage::new(&mut ctx,
                tx_keys,
                rx_disp,
                rx_disp_notify,
            HEIGHT, WIDTH))
        });
        // while window.is_open() && !window.is_key_down(Key::Escape) {
        //     let cpu_keys = convert_keys(&window);
        //     match tx_keys.try_send(cpu_keys) {
        //         Ok(..) => {}
        //         Err(TrySendError::Full(..)) => {} //skipped input
        //         Err(TrySendError::Disconnected(..)) => break,
        //     }

        //     match rx_disp_notify.try_recv() {
        //         Ok(..) => match rx_disp.recv() {
        //             Ok((display_buf, height, width)) => {
        //                 buffer[..height * width].copy_from_slice(&display_buf[..]);
        //                 window
        //                     .update_with_buffer(&buffer, width, height)
        //                     .context("Updating minifb display buffer failed")?;
        //             }
        //             Err(RecvError) => break,
        //         },
        //         Err(TryRecvError::Empty) => {
        //             window.update();
        //         }
        //         Err(TryRecvError::Disconnected) => break,
        //     }
        //     perf_io.wait();
        //     if !ticker_fps.wait_nonblocking() && debug >= 1 {
        //         println!("frames per second       (fps): {}", perf_io.get_fps());
        //     }
        // }
        println!("Exiting");
        // drop(rx_disp);
        // drop(tx_keys);
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

use miniquad::{Buffer, Pipeline, Bindings, BufferType, Texture, FilterMode, Shader, BufferLayout, VertexAttribute, VertexFormat, EventHandler};

#[repr(C)]
struct Vec2 {
    x: f32,
    y: f32,
}
#[repr(C)]
struct Vertex {
    pos: Vec2,
    uv: Vec2,
}

struct Stage {
    pipeline: Pipeline,
    bindings: Bindings,
    tx_keys: mpsc::SyncSender<[VKey; 16]>,
    rx_disp: mpsc::Receiver<(Vec<u32>, usize, usize)>,
    rx_disp_notify: mpsc::Receiver<()>,
    buffer: Vec<u8>,
    height: usize,
    width: usize,
    cpu_keys: [VKey; 16],
}

impl Stage {
    pub fn new(ctx: &mut miniquad::Context,
        tx_keys: mpsc::SyncSender<[VKey; 16]>,
        rx_disp: mpsc::Receiver<(Vec<u32>, usize, usize)>,
        rx_disp_notify: mpsc::Receiver<()>,
        height: usize,
        width: usize,
    ) -> Stage {
        #[rustfmt::skip]
        let vertices: [Vertex; 4] = [
            Vertex { pos : Vec2 { x: -1., y: -1. }, uv: Vec2 { x: 0., y: 1. } },
            Vertex { pos : Vec2 { x:  1., y: -1. }, uv: Vec2 { x: 1., y: 1. } },
            Vertex { pos : Vec2 { x:  1., y:  1. }, uv: Vec2 { x: 1., y: 0. } },
            Vertex { pos : Vec2 { x: -1., y:  1. }, uv: Vec2 { x: 0., y: 0. } },
        ];
        let vertex_buffer = Buffer::immutable(ctx, BufferType::VertexBuffer, &vertices);

        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let index_buffer = Buffer::immutable(ctx, BufferType::IndexBuffer, &indices);

        let pixels: Vec<u8> = vec![255; height * width * 4];
        let texture = Texture::from_rgba8(ctx, width as u16,  height as u16, &pixels);
        texture.set_filter(ctx, FilterMode::Nearest);

        let bindings = Bindings {
            vertex_buffers: vec![vertex_buffer],
            index_buffer: index_buffer,
            images: vec![texture],
        };

        let shader = Shader::new(ctx, shader::VERTEX, shader::FRAGMENT, shader::meta()).unwrap();

        let pipeline = Pipeline::new(
            ctx,
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("pos", VertexFormat::Float2),
                VertexAttribute::new("uv", VertexFormat::Float2),
            ],
            shader,
        );

        Stage { pipeline, bindings, tx_keys, rx_disp, rx_disp_notify, buffer: Vec::new(), cpu_keys: [VKey::Up;16], height, width} 
    }
}

impl EventHandler for Stage {
    fn key_up_event(&mut self, _ctx: &mut miniquad::Context, _keycode: miniquad::KeyCode, _keymods: miniquad::KeyMods) {
        let idx = match _keycode{
            miniquad::KeyCode::X => 0,
            miniquad::KeyCode::Key1 => 1,
            miniquad::KeyCode::Key2 => 2,
            miniquad::KeyCode::Key3 => 3,
            miniquad::KeyCode::Q => 4,
            miniquad::KeyCode::W => 5,
            miniquad::KeyCode::E => 6,
            miniquad::KeyCode::A => 7,
            miniquad::KeyCode::S => 8,
            miniquad::KeyCode::D => 9,
            miniquad::KeyCode::Z => 10,
            miniquad::KeyCode::C => 11,
            miniquad::KeyCode::Key4 => 12,
            miniquad::KeyCode::R => 13,
            miniquad::KeyCode::F => 14,
            miniquad::KeyCode::V => 15,
            _ => 16,
        };
        if idx < 16 { self.cpu_keys[idx] = VKey::Up;}
        if matches!(_keycode, miniquad::KeyCode::Escape) {
            _ctx.quit();
        }
    }
    fn key_down_event(&mut self, _ctx: &mut miniquad::Context, _keycode: miniquad::KeyCode, _keymods: miniquad::KeyMods, _repeat: bool) {
        let idx = match _keycode{
            miniquad::KeyCode::X => 0,
            miniquad::KeyCode::Key1 => 1,
            miniquad::KeyCode::Key2 => 2,
            miniquad::KeyCode::Key3 => 3,
            miniquad::KeyCode::Q => 4,
            miniquad::KeyCode::W => 5,
            miniquad::KeyCode::E => 6,
            miniquad::KeyCode::A => 7,
            miniquad::KeyCode::S => 8,
            miniquad::KeyCode::D => 9,
            miniquad::KeyCode::Z => 10,
            miniquad::KeyCode::C => 11,
            miniquad::KeyCode::Key4 => 12,
            miniquad::KeyCode::R => 13,
            miniquad::KeyCode::F => 14,
            miniquad::KeyCode::V => 15,
            _ => 16,
        };
        if idx < 16 { self.cpu_keys[idx] = VKey::Down;}
    }
    fn update(&mut self, _ctx: &mut miniquad::Context) {
        // let mut cpu_keys = [VKey::Up; 16];
        match self.tx_keys.try_send(self.cpu_keys) {
            Ok(..) => {}
            Err(TrySendError::Full(..)) => {} //skipped input
            Err(TrySendError::Disconnected(..)) => {_ctx.quit(); return},
        }

        match self.rx_disp_notify.try_recv() {
            Ok(..) => match self.rx_disp.recv() {
                Ok((display_buf, height, width)) => {
                    self.buffer.clear();
                    for h in 0..height {
                        for w in 0..width {
                            let val = display_buf[w + width * h];
                            self.buffer.push((val >> 16 & 0xFF) as u8);
                            self.buffer.push((val >> 8 & 0xFF) as u8);
                            self.buffer.push((val & 0xFF) as u8);
                            self.buffer.push(0xFF);
                        }
                    }
                    if height != self.height || width != self.width {
                        //self.bindings.images[0].resize(_ctx, width as u32, height as u32, Some(&self.buffer));

                        let texture = Texture::from_rgba8(_ctx, width as u16,  height as u16, &self.buffer);
                        texture.set_filter(_ctx, FilterMode::Nearest);
                        self.bindings.images[0] = texture;

                        self.height = height;
                        self.width = width;
                    }
                    self.bindings.images[0].update(_ctx, &self.buffer);
                }
                Err(RecvError) => {_ctx.quit(); return},
            },
            Err(TryRecvError::Empty) => {
            }
            Err(TryRecvError::Disconnected) => {_ctx.quit(); return},
        }
    }


    fn draw(&mut self, ctx: &mut miniquad::Context) {
        ctx.begin_default_pass(Default::default());
        //self.bindings.images[0].update(ctx, &self.buffer);

        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_bindings(&self.bindings);
        ctx.apply_uniforms(&shader::Uniforms {
            offset: (0., 0.),
        });
        ctx.draw(0, 6, 1);
        ctx.end_render_pass();

        ctx.commit_frame();
    }
}

fn main() {
}

mod shader {
    use miniquad::*;

    pub const VERTEX: &str = r#"#version 100
    attribute vec2 pos;
    attribute vec2 uv;
    uniform vec2 offset;
    varying lowp vec2 texcoord;
    void main() {
        gl_Position = vec4(pos, 0, 1);
        texcoord = uv;
    }"#;

    pub const FRAGMENT: &str = r#"#version 100
    varying lowp vec2 texcoord;
    uniform sampler2D tex;
    void main() {
        gl_FragColor = texture2D(tex, texcoord);
    }"#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec!["tex".to_string()],
            uniforms: UniformBlockLayout {
                uniforms: vec![UniformDesc::new("offset", UniformType::Float2)],
            },
        }
    }

    #[repr(C)]
    pub struct Uniforms {
        pub offset: (f32, f32),
    }
}