//! Where the vertical sync begins and ends, measured on rung 0: the
//! sync-tip leg (`vid_sync_l`) read every half-step through one whole
//! frame of the standard world with rendering on, every run of it
//! printed as (row, dot) start to (row, dot) end with its length in
//! dots. A picture row's run is the horizontal sync; the runs that are
//! longer are the vertical sync's broad pulses, and the row and dot
//! the first of them begins on is the number the encoder
//! (ntsc-source-nes's `segment`) and the bench's `poll-line.py` had
//! been assuming at line granularity: rows 245..247 from dot 0. The
//! bench measured the part's poll six tenths of a line earlier than
//! the model's on two screens against that assumption
//! (nes-bench/docs/mario-dissection.md), which is what this settles.
//!
//! Also printed: the blanking leg's runs across the same rows, so the
//! serration's shape is read and not inferred from the tip alone.

use v2c02_dots::{standard_world, Leg, Taps};

fn main() {
    let mut h = standard_world();
    let taps = Taps::new(&h);
    // To the frame's last dot, as capture() does, so the frame read is
    // whole: 261 (pre-render) first, then 0..=260.
    while !(taps.bus(&h, &taps.vpos) == 261 && taps.bus(&h, &taps.hpos) == 340) {
        h.half_step();
    }
    let mut trace: Vec<(u16, u16, bool, bool)> = Vec::new();
    let mut seen_picture = false;
    loop {
        h.half_step();
        let hp = taps.bus(&h, &taps.hpos) as u16;
        let vp = taps.bus(&h, &taps.vpos) as u16;
        if vp < 261 {
            seen_picture = true;
        } else if seen_picture {
            break;
        }
        let m = taps.leg_mask(&h);
        trace.push((vp, hp, m & (1 << Leg::SyncTip as u16) != 0, m & (1 << Leg::Blank as u16) != 0));
    }
    let per_dot = trace.len() as f64 / (262.0 * 341.0);
    println!("one frame: {} half-steps, {per_dot:.2} a dot", trace.len());
    for (name, pick) in [("sync tip (vid_sync_l)", 2usize), ("blanking leg (vid_sync_h)", 3)] {
        println!("{name} runs, (row, dot) first half-step asserted .. (row, dot) last, length in dots; picture rows 0..=239 summarised:");
        let on = |t: &(u16, u16, bool, bool)| if pick == 2 { t.2 } else { t.3 };
        let mut i = 0;
        let mut picture: Vec<(u16, u16, u16, f64)> = Vec::new();
        while i < trace.len() {
            if !on(&trace[i]) {
                i += 1;
                continue;
            }
            let start = i;
            while i < trace.len() && on(&trace[i]) {
                i += 1;
            }
            let (a, b) = (trace[start], trace[i - 1]);
            let len = (i - start) as f64 / per_dot;
            if a.0 < 240 && b.0 < 240 {
                picture.push((a.0, a.1, b.1, len));
            } else {
                println!("  ({}, {}) .. ({}, {})  {len:.1} dots", a.0, a.1, b.0, b.1);
            }
        }
        let dots: std::collections::BTreeSet<(u16, u16)> = picture.iter().map(|p| (p.1, p.2)).collect();
        let lens: std::collections::BTreeSet<i64> = picture.iter().map(|p| (p.3 * 10.0).round() as i64).collect();
        println!("  picture rows: {} runs, start..end dots {:?}, lengths {:?} tenths", picture.len(), dots, lens);
    }
}
