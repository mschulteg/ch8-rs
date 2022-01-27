ch8-rs
======

CHIP-8 emulator that also supports Super-CHIP8 and XO-CHIP (with audio).
The main purpose of this project is for me to learn a bit about Rust.
Different quirks are not yet implemented and only a bunch of roms are tested.

## Example

``` console
cargo run --release -- "roms/xochip/superneatboy.ch8" -d --colors 100010,E6E6FA,FF1493,FF1493 --fps-limit 60 --ips-limit 100000
```

## Usage

``` console
$ cargo run --release -- --help

ch8-rs 0.1.1
Moritz Schulte <mschulte.g@gmail.com>
Chip 8 emulator

USAGE:
    ch8-rs [FLAGS] [OPTIONS] <rom_path>

FLAGS:
    -d                      Turn debugging information on
    -h, --help              Prints help information
        --no-skip-frames    Do not skip frames - Frames are skipped by default
    -p, --perf-counter      Show performance counter
    -V, --version           Prints version information

OPTIONS:
        --colors <COLORS>    Four or two colors provided as four or two 6 digit hex numbers separated with a comma. For
                             chip8 and super-chip8 programs, two colors can be specified, representing the background
                             and foreground colors.
                             Example: 000000,FFFFFF sets the background color to black and the foreground color to
                             white.
                             For xo-chip programs, four colors can be provided for the four possible combinations of
                             values in the two drawing planes.
                             Example: 000000,FF0000,00FF00,0000FF sets the colors for "background, plane1, plane2, both
                             planes blended" or in other words: it sets the "background, fill1, fill2, blend" colors
                              [default: 00AA4400,00FFAA00,00AAAAAA,00000000]
        --fps-limit <FPS>    Limit loop that polls input and draws output
        --ipf-limit <IPF>    Limits instructions per frame
        --ips-limit <IPS>    Limits instructions per second

ARGS:
    <rom_path>    Path to rom file
```

## Issues
- The fps limiter is unprecise under windows

## Requirements

### Rust

This program targets the latest stable version of Rust 1.48.0 or later.
