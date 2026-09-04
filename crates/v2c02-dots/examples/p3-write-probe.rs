//! The $2006/$2007 write path, measured: after a $2006 pair pointing
//! at a fresh sprite-palette row, write four known bytes with a chosen
//! idle after the pair and a chosen idle between the writes, then read
//! the row back paced and print what held, beside what the harness saw
//! leave for the CHR bus. One row per (pair idle, write idle) case.
//! Rendering is off throughout. Measurement only.

use v2c02_dots::{standard_world, Taps};

fn main() {
    let mut h = standard_world();
    h.write(1, 0x00);
    h.wait(48);
    let taps = Taps::new(&h);
    while taps.bus(&h, &taps.vpos) != 242 {
        h.half_step();
    }
    let values = [0x21u8, 0x11, 0x01, 0x2a];
    println!("case: pair idle / write idle -> held at $3F11..$3F14 (wrote 21 11 01 2a); CHR-bus writes seen");
    for (pair_idle, write_idle) in [(0u64, 0u64), (0, 24), (24, 0), (24, 24), (48, 0), (48, 24), (96, 0), (96, 24), (192, 0), (192, 24)] {
        // Clear the row to a known value first, paced generously.
        h.write(6, 0x3f);
        h.wait(48);
        h.write(6, 0x11);
        h.wait(192);
        for _ in 0..4 {
            h.write(7, 0x00);
            h.wait(48);
        }
        h.wait(192);
        let before = h.vram_writes.len();
        h.write(6, 0x3f);
        h.wait(pair_idle);
        h.write(6, 0x11);
        h.wait(pair_idle);
        for &v in &values {
            h.write(7, v);
            h.wait(write_idle);
        }
        h.wait(192);
        let stray: Vec<String> = h.vram_writes[before..].iter().map(|(a, v)| format!("${a:04x}<-{v:02x}")).collect();
        h.write(6, 0x3f);
        h.wait(48);
        h.write(6, 0x11);
        h.wait(96);
        let mut held = [0u8; 4];
        for x in held.iter_mut() {
            *x = h.read(7);
            h.wait(48);
        }
        println!(
            "{pair_idle:3} / {write_idle:2} -> {}   {}",
            held.iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" "),
            if stray.is_empty() { "none".to_string() } else { stray.join(" ") }
        );
    }
}
