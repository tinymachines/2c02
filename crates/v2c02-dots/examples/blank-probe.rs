//! What the chip shows with rendering off, measured on rung 0 in the
//! standard world: the backdrop, or the palette entry v points at when
//! v sits in $3F00..$3FFF, and how a mid-line $2006 pair or $2007 write
//! moves it. The quirk `full_palette.nes` paints with.
//!
//! One capture of the first `BLANK_ROWS` rows of a frame with rendering
//! off (mask $00) and `BLANK_WRITES` at their dots; each row's colour
//! runs are printed with the dot each write started on, and the palette
//! as the chip holds it (read back paced in vblank, entries the world
//! never wrote included) with the captured rows are written to
//! goldens/blank.bin (32 palette bytes, then BLANK_ROWS x 341 colour
//! bytes) for `v2c02-fast`'s blank gate.

use v2c02_dots::{capture_with_writes, read_back_palette_and_oam, standard_world, BLANK_ROWS, BLANK_WRITES};

fn main() {
    let mut h = standard_world();
    let (pal, _) = read_back_palette_and_oam(&mut h);
    println!("palette as held: {}", pal.iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" "));
    h.write(1, 0x00);
    h.wait(48);
    let (cap, started) = capture_with_writes(&mut h, BLANK_ROWS, &BLANK_WRITES);
    let first = h.half_steps - cap.trace.len() as u64;
    println!("half-steps per dot: {:.2}", cap.trace.len() as f64 / (BLANK_ROWS as f64 * 341.0));
    for (wr, &s) in BLANK_WRITES.iter().zip(&started) {
        let (hp, vp, _, _) = cap.trace[(s - first) as usize];
        println!("write ${:04x} <- {:02x} scheduled ({}, {}) started ({vp}, {hp})", 0x2000 + wr.reg as u16, wr.val, wr.vpos, wr.hpos);
    }
    for row in 58..BLANK_ROWS {
        let mut runs: Vec<(usize, u8)> = Vec::new();
        for dot in 1..=256 {
            let (c, _) = cap.dots.at(row, dot);
            if runs.last().map(|r| r.1) != Some(c) {
                runs.push((dot, c));
            }
        }
        println!("row {row}: {}", runs.iter().map(|(d, c)| format!("{d}:{c:02x}")).collect::<Vec<_>>().join(" "));
    }
    // The emphasis attenuation (vid_emph) on row 67: the hpos it first
    // and last pulses on, per half-step trace.
    let on: Vec<u16> = cap.trace.iter().filter(|t| t.1 == 67 && t.3).map(|t| t.0).collect();
    println!("row 67 vid_emph high at hpos {:?}..{:?} ({} half-steps)", on.first(), on.last(), on.len());
    // The DAC's legs beside the palette bus: on row 60 the pair's colour
    // change and on row 67 the emphasis, per half-step, so the two
    // paths' skews are read at the same point (the video).
    for (row, from, to) in [(60u16, 48u16, 60u16), (67, 98, 108), (67, 198, 208)] {
        let mut last = None;
        for t in cap.trace.iter().filter(|t| t.1 == row && t.0 >= from && t.0 < to) {
            let k = (t.2, t.3);
            if last != Some(k) {
                println!("row {row} hpos {}: legs {:011b} emph {}", t.0, t.2, t.3 as u8);
                last = Some(k);
            }
        }
    }
    let mut bytes = Vec::with_capacity(32 + BLANK_ROWS * 341);
    bytes.extend_from_slice(&pal);
    for row in 0..BLANK_ROWS {
        for dot in 0..341 {
            bytes.push(cap.dots.at(row, dot).0);
        }
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../goldens");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("blank.bin"), &bytes).unwrap();
    println!("wrote goldens/blank.bin ({} bytes)", bytes.len());
}
