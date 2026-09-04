//! Two measurements for P3's datapath, both off rung 0 in the standard
//! world: (1) the chip's own pixel index (`pixel_color0..3`, the 4-bit
//! palette address the mux selects) beside the colour on `pal_d`, per
//! dot for the first rows, so the pattern/attribute stream can be held
//! to the model BEFORE the palette lookup; (2) the palette RAM read back
//! through $2007 during vblank, so what the chip holds is known rather
//! than assumed from the writes. Measurement only.

use v2c02_dots::{standard_world, Taps};

fn main() {
    let mut h = standard_world();
    let taps = Taps::new(&h);
    let nl = h.ppu.engine.netlist().clone();
    let n = |s: &str| nl.node(s).unwrap_or_else(|| panic!("node {s}"));
    let pix: Vec<_> = (0..4).map(|i| n(&format!("pixel_color{i}"))).collect();
    let spat: Vec<_> = (0..2).map(|i| n(&format!("selected_pat{i}"))).collect();
    let sattr: Vec<_> = (0..2).map(|i| n(&format!("selected_attr{i}"))).collect();

    while !(taps.bus(&h, &taps.vpos) == 261 && taps.bus(&h, &taps.hpos) == 340) {
        h.half_step();
    }
    // Rows 0..3, dots (hpos) 0..48: index, pat, attr, colour at the
    // pclk1 sample.
    type Sample = (u16, u8, u8, u8, u8);
    let mut seen_pclk1 = false;
    let mut rows: Vec<Vec<Sample>> = vec![Vec::new(); 3];
    loop {
        h.half_step();
        let hp = taps.bus(&h, &taps.hpos) as u16;
        let vp = taps.bus(&h, &taps.vpos) as usize;
        if vp >= 3 && vp != 261 {
            break;
        }
        let p1 = h.ppu.engine.is_high(taps.pclk1);
        if p1 && seen_pclk1 && vp < 3 && hp < 48 {
            rows[vp].push((
                hp,
                taps.bus(&h, &pix) as u8,
                taps.bus(&h, &spat) as u8,
                taps.bus(&h, &sattr) as u8,
                taps.bus(&h, &taps.pal_d) as u8,
            ));
            seen_pclk1 = false;
        } else {
            seen_pclk1 = p1;
        }
    }
    for (r, row) in rows.iter().enumerate() {
        println!("row {r}: hpos:idx/pat/attr=colour");
        for (hp, idx, pat, attr, col) in row {
            print!("{hp}:{idx:x}/{pat}/{attr}={col:02x} ");
            if hp % 8 == 7 {
                println!();
            }
        }
        println!();
    }

    // Palette read-back in vblank: run to vpos 241, set $3F00, read 32.
    while taps.bus(&h, &taps.vpos) != 242 {
        h.half_step();
    }
    h.write(6, 0x3f);
    h.write(6, 0x00);
    let pal: Vec<u8> = (0..32).map(|_| h.read(7)).collect();
    println!("palette RAM via $2007: {}", pal.iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" "));
}
