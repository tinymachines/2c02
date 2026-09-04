//! Two questions about the palette path, measured per half-step on
//! rung 0 in the standard world:
//! (1) What does the palette RAM hold? Read back through $2007 with an
//!     access-width of idle between reads (the pacing P2 found the
//!     register file needs), twice, so an unstable readback shows.
//! (2) How do `pixel_color0..3` (the mux's index), `pal_ptr0..4` (the
//!     address the RAM sees) and `pal_d0..5_out` (the bus the capture
//!     samples) relate within a dot? Printed per half-step for the
//!     first dots of row 0 with the pclk phase.
//! Measurement only.

use v2c02_dots::{standard_world, Taps};

fn main() {
    let mut h = standard_world();
    let taps = Taps::new(&h);
    let nl = h.ppu.engine.netlist().clone();
    let n = |s: &str| nl.node(s).unwrap_or_else(|| panic!("node {s}"));
    let pix: Vec<_> = (0..4).map(|i| n(&format!("pixel_color{i}"))).collect();
    let ptr: Vec<_> = (0..5).map(|i| n(&format!("pal_ptr{i}"))).collect();
    let pclk0 = n("pclk0");

    // (1) Paced palette read-back in vblank, twice.
    while taps.bus(&h, &taps.vpos) != 242 {
        h.half_step();
    }
    for pass in 0..2 {
        h.write(6, 0x3f);
        h.wait(24);
        h.write(6, 0x00);
        h.wait(24);
        let mut pal = Vec::new();
        for _ in 0..32 {
            pal.push(h.read(7));
            h.wait(24);
        }
        println!(
            "palette RAM via paced $2007, pass {pass}: {}",
            pal.iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" ")
        );
    }

    // (2) Per half-step through row 0, hpos 0..=20.
    while !(taps.bus(&h, &taps.vpos) == 261 && taps.bus(&h, &taps.hpos) == 340) {
        h.half_step();
    }
    println!("row 0 per half-step: hpos p0p1 idx ptr pal_d");
    loop {
        h.half_step();
        let hp = taps.bus(&h, &taps.hpos);
        let vp = taps.bus(&h, &taps.vpos);
        if vp != 0 {
            if vp == 261 {
                continue;
            }
            break;
        }
        if hp > 20 {
            break;
        }
        println!(
            "{hp:3} {}{}  {:x}  {:02x}  {:02x}",
            h.ppu.engine.is_high(pclk0) as u8,
            h.ppu.engine.is_high(taps.pclk1) as u8,
            taps.bus(&h, &pix),
            taps.bus(&h, &ptr),
            taps.bus(&h, &taps.pal_d)
        );
    }
}
