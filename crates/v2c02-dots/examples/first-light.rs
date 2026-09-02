//! First light: a full visible frame captured off the switch-level
//! chip, encoded to composite by ntsc-crt's NES source, decoded on
//! Rung A, displayed through the five CRT stages, written as PPM.
//! Run with --release; the capture alone is ~650k half-steps.

use std::io::Write as _;

use ntsc_crt::{CrtParams, CrtPipeline, GeometryParams, MaskParams};
use ntsc_decode::Decoder;
use ntsc_grid::Phase;
use ntsc_source_nes::{burst_axis_offset, encode_frame, levels, Levels};
use v2c02_dots::{capture, standard_world};

fn main() {
    let mut h = standard_world();
    eprintln!("world ready at half-step {}", h.half_steps);
    let cap = capture(&mut h, 240);
    eprintln!("captured a frame at half-step {} ({} trace steps)", h.half_steps, cap.trace.len());

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../goldens");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("p1-dots.bin"), &cap.dots.colour).unwrap();
    std::fs::write(
        out.join("p1-dots.stamp.txt"),
        format!(
            "2c02 P1 dot-stream golden: one visible frame off rung 0\n\
             world: v2c02_dots::standard_world (vram = ((a>>4)^a)&0xff, the 16-entry palette, bg on)\n\
             recorded: 2026-09-02 by examples/first-light.rs at {} half-steps\n",
            h.half_steps
        ),
    )
    .unwrap();

    let frame = encode_frame(&Levels::transcribed(), &cap.dots, Phase::new(0));
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
    let path = out.join("p1-first-light.ppm");
    std::fs::write(&path, ppm).unwrap();
    println!("wrote {} ({} x {})", path.display(), display.width, display.height);
}
