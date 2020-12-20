mod cpu;
mod emulator;
mod perf;

use emulator::Emulator;

use std::fs::File;
use std::io::BufReader;
use std::io::Read;

fn main() {
    let path = std::env::args().nth(1).expect("No file given");

    let f = File::open(path).unwrap();
    let mut buf_reader = BufReader::new(f);
    let mut code = Vec::<u8>::new();
    buf_reader
        .read_to_end(&mut code)
        .expect("Could not read file to end");

    let emulator = Emulator::new()
        .with_skip_frames()
        .with_fps_limit(60.0);//.with_debug();
        //.with_ips_limit(10000.0);
    emulator.run(&code[..]);
}
