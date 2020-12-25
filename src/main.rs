mod cpu;
mod emulator;
mod perf;

use emulator::Emulator;

use std::fs::File;
use std::io::BufReader;
use std::io::Read;

use clap::{App, Arg};


fn main() {
    let matches = App::new("Chip8Emu")
        .version("1.0")
        .author("Moritz Schulte")
        .about("Chip 8 emulator")
        .arg(
            Arg::with_name("rom_path")
                .help("Path to rom file")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("debug")
                .short("d")
                .multiple(true)
                .help("Turn debugging information on"),
        )
        .arg(
            Arg::with_name("fps-limit")
                .long("fps-limit")
                .value_name("FPS")
                .help("Limit loop that polls input and draws output")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ips-limit")
                .long("ips-limit")
                .value_name("IPS")
                .help("Limits instructions per second")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ipf-limit")
                .long("ipf-limit")
                .value_name("IPF")
                .help("Limits instructions per frame")
                .takes_value(true)
                .conflicts_with("ips-limit")
                .requires("fps-limit")
        )
        .arg(
            Arg::with_name("no-skip-frames")
                .long("no-skip-frames")
                .help("Do not skip frames - Frames are skipped by default")
        )
        .arg(
            Arg::with_name("perf-counter")
                .long("perf-counter")
                .short("p")
                .help("Show performance counter")
        )
        .get_matches();
    
    let path = matches.value_of("rom_path").expect("No file given");
    let debug = matches.occurrences_of("debug");
    let fps_limit = matches.value_of("fps-limit").and_then(|string| string.parse::<f64>().ok());
    let mut ips_limit = matches.value_of("ips-limit").and_then(|string| string.parse::<f64>().ok());
    let ipf_limit = matches.value_of("ipf-limit").and_then(|string| string.parse::<f64>().ok());
    let skip_frames = !matches.is_present("no-skip-frames");

    if let Some(ipf_limit) = ipf_limit {
        if let Some(fps_limit) = fps_limit {
            ips_limit = Some(fps_limit * ipf_limit);
        }
    }

    let f = File::open(path).unwrap();
    let mut buf_reader = BufReader::new(f);
    let mut code = Vec::<u8>::new();
    buf_reader
        .read_to_end(&mut code)
        .expect("Could not read file to end");

    let emulator = Emulator::new()
        .with_skip_frames(skip_frames)
        .with_fps_limit(fps_limit)//.with_debug();
        .with_ips_limit(ips_limit)
        .with_debug(debug);

    emulator.run(&code[..]);
}
