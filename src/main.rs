mod cpu;
mod emulator;
mod perf;
mod sound;
use emulator::Emulator;

use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::u32;

use anyhow::Context;
use clap::{crate_version, crate_authors, crate_name, App, Arg};

fn parse_colors(input: &str) -> [u32; 4] {
    let mut colors = [0u32; 4];
    for (i, ccode) in input.split(',').take(4).enumerate() {
        colors[i] = u32::from_str_radix(ccode, 16).unwrap()
    }
    colors
}

fn main() -> Result<(), anyhow::Error> {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
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
                .requires("fps-limit"),
        )
        .arg(
            Arg::with_name("colors")
                .long("colors")
                .value_name("COLORS")
                .help(
                    "Four or two colors provided as four or two 6 digit hex numbers separated with a comma. \
                     For chip8 and super-chip8 programs, two colors can be specified, representing the background \
                     and foreground colors.\n\
                     Example: 000000,FFFFFF sets the background color to black and the foreground color to white.\n\
                     For xo-chip programs, four colors can be provided for the four possible combinations \
                     of values in the two drawing planes.\n\
                     Example: 000000,FF0000,00FF00,0000FF sets the colors for \"background, plane1, plane2, both planes blended\" or \
                     in other words: it sets the \"background, fill1, fill2, blend\" colors\n",
                )
                .takes_value(true)
                .default_value("00AA4400,00FFAA00,00AAAAAA,00000000"),
        )
        .arg(
            Arg::with_name("no-skip-frames")
                .long("no-skip-frames")
                .help("Do not skip frames - Frames are skipped by default"),
        )
        .arg(
            Arg::with_name("perf-counter")
                .long("perf-counter")
                .short("p")
                .help("Show performance counter"),
        )
        .get_matches();

    let path = matches.value_of("rom_path").unwrap();
    let debug = matches.occurrences_of("debug");
    let fps_limit = matches
        .value_of("fps-limit")
        .and_then(|string| string.parse::<f64>().ok());
    let mut ips_limit = matches
        .value_of("ips-limit")
        .and_then(|string| string.parse::<f64>().ok());
    let ipf_limit = matches
        .value_of("ipf-limit")
        .and_then(|string| string.parse::<f64>().ok());
    let skip_frames = !matches.is_present("no-skip-frames");

    if let Some(ipf_limit) = ipf_limit {
        if let Some(fps_limit) = fps_limit {
            ips_limit = Some(fps_limit * ipf_limit);
        }
    }

    let colors = matches
        .value_of("colors")
        .map(|colors| parse_colors(colors));

    let f = File::open(path).with_context(|| format!("Rom file {} is cannot be opened", path))?;
    let mut buf_reader = BufReader::new(f);
    let mut code = Vec::<u8>::new();
    buf_reader
        .read_to_end(&mut code)
        .context("Could not read rom file to end")?;

    let emulator = Emulator::new()
        .with_skip_frames(skip_frames)
        .with_fps_limit(fps_limit)
        .with_ips_limit(ips_limit)
        .with_colors(colors)
        .with_debug(debug);

    emulator.run(code)?;
    Ok(())
}
