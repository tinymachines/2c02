//! A dot golden through the family's own signal path: load a captured
//! DotFrame golden (`goldens/p1-dots.bin`, or the dots of
//! `goldens/p3-sprites.bin` / `goldens/p3-scroll.bin`), encode it to
//! composite with ntsc-source-nes, decode it on Rung A, run the five CRT
//! stages, and write a PPM. The picture is what the switch-level chip
//! drew, seen the way a television would show it.
//!
//!   cargo run --release -p v2c02-dots --example render-golden -- <golden.bin> <out.ppm>

use std::io::Write as _;

use nes_bus::{DotFrame, FrameParity, DOTS_PER_LINE, LINES};
use ntsc_crt::{CrtParams, CrtPipeline, GeometryParams, MaskParams};
use ntsc_decode::Decoder;
use ntsc_grid::Phase;
use ntsc_source_nes::{burst_axis_offset, encode_frame, levels, Levels};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (input, output) = (&args[1], &args[2]);
    let bytes = std::fs::read(input).expect("golden file");
    let n = LINES * DOTS_PER_LINE;
    // The dots are the trailing 262 x 341 bytes of any of the goldens.
    let colour = &bytes[bytes.len() - n..];
    let mut dots = DotFrame::filled(FrameParity::Even, 0x0f, 0);
    for (i, &c) in colour.iter().enumerate() {
        dots.set(i / DOTS_PER_LINE, i % DOTS_PER_LINE, c, 0);
    }
    let frame = encode_frame(&Levels::transcribed(), &dots, Phase::new(0));
    let dec = Decoder::transcribed(burst_axis_offset(), levels::LOW[1], levels::HIGH[2]);
    let rgb = dec.decode(&frame, 0, 240, 2048);
    let mut params = CrtParams::authored(3);
    params.mask = Some(MaskParams { pitch: 1, off_gain: 0.3 });
    params.geometry = Some(GeometryParams { barrel_k: 0.03, corner_radius: 12.0 });
    let mut pipe = CrtPipeline::new(params);
    let display = pipe.process(&rgb);
    let mut ppm = Vec::new();
    write!(ppm, "P6\n{} {}\n255\n", display.width, display.height).unwrap();
    for px in display.to_rgba8().chunks_exact(4) {
        ppm.extend_from_slice(&px[..3]);
    }
    std::fs::write(output, ppm).unwrap();
    println!("wrote {output} ({} x {})", display.width, display.height);
}
