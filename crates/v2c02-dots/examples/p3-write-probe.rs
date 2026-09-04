//! The $2006/$2007 write path, measured. Three tables, rendering off:
//!
//! 1. After a $2006 pair pointing at a fresh sprite-palette row, write
//!    four known bytes back to back or with idle between them, with the
//!    data bus held after each write (the harness default) or floated
//!    at the end of each access (its earlier behaviour), on a row below
//!    and above $20, and read the row back paced.
//! 2. The $2006 load: a $2006 pair, a gap of g half-steps, one $2007
//!    write; where the byte lands says whether v had loaded.
//!
//! Measurement only.

use v2c02_dots::{standard_world, Taps};
use v2c02_sim::harness::Harness;

fn to_vblank(h: &mut Harness) {
    let taps = Taps::new(h);
    while taps.bus(h, &taps.vpos) != 242 {
        h.half_step();
    }
}

fn read_row(h: &mut Harness, lo: u8, n: usize) -> Vec<u8> {
    h.write(6, 0x3f);
    h.wait(48);
    h.write(6, lo);
    h.wait(96);
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(h.read(7));
        h.wait(48);
    }
    out
}

fn clear_row(h: &mut Harness, lo: u8) {
    h.write(6, 0x3f);
    h.wait(48);
    h.write(6, lo);
    h.wait(192);
    for _ in 0..4 {
        h.write(7, 0x00);
        h.wait(48);
    }
    h.wait(192);
}

fn hex(v: &[u8]) -> String {
    v.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

fn main() {
    let mut h = standard_world();
    h.write(1, 0x00);
    h.wait(48);
    to_vblank(&mut h);
    let values = [0x21u8, 0x11, 0x01, 0x2a];

    println!("1. bus after write / idle between writes / row -> held (wrote 21 11 01 2a)");
    for (float_after, write_idle, row) in [
        (false, 0u64, 0x11u8),
        (false, 24, 0x11),
        (true, 0, 0x11),
        (true, 24, 0x11),
        (false, 48, 0x11),
        (false, 96, 0x11),
        (false, 192, 0x11),
        (false, 24, 0x21),
        (false, 96, 0x21),
    ] {
        h.float_after_access = false;
        clear_row(&mut h, row);
        h.float_after_access = float_after;
        h.write(6, 0x3f);
        h.wait(48);
        h.write(6, row);
        h.wait(96);
        for &v in &values {
            h.write(7, v);
            h.wait(write_idle);
        }
        h.wait(192);
        h.float_after_access = false;
        let held = read_row(&mut h, row, 4);
        println!(
            "   {} / {:3} / $3F{row:02x} -> {}",
            if float_after { "floated" } else { "held   " },
            write_idle,
            hex(&held)
        );
    }

    println!("2. delayed $2006 load: gap g half-steps between the pair and one $2007 write of 5a; where it landed");
    h.float_after_access = false;
    for gap in [0u64, 2, 4, 6, 8, 10, 12, 16, 24, 48] {
        clear_row(&mut h, 0x11);
        // Park v somewhere else first so a stale write is visible.
        h.write(6, 0x3f);
        h.wait(48);
        h.write(6, 0x15);
        h.wait(96);
        let before = h.vram_writes.len();
        h.write(6, 0x3f);
        h.wait(48);
        h.write(6, 0x11);
        h.wait(gap);
        h.write(7, 0x5a);
        h.wait(192);
        let stray: Vec<String> = h.vram_writes[before..].iter().map(|(a, v)| format!("${a:04x}<-{v:02x}")).collect();
        let row11 = read_row(&mut h, 0x11, 2);
        let row15 = read_row(&mut h, 0x15, 2);
        println!(
            "   g={gap:2}: $3F11.. {}  $3F15.. {}  CHR-bus writes: {}",
            hex(&row11),
            hex(&row15),
            if stray.is_empty() { "none".into() } else { stray.join(" ") }
        );
    }
}
