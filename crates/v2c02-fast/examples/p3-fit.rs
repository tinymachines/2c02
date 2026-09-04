//! The measurement the gate's golden offset is pinned from: render the
//! standard world with the stepper and, for each candidate offset, count
//! the visible dots where the stepper's dot d disagrees with the rung 0
//! golden's dot d + offset. The minimum is what tests/p3.rs pins as
//! GOLDEN_PIXEL_OFFSET. Prints the first residual disagreements at the
//! best offset so a mismatch is named rather than averaged away.

use nes_bus::{ACTIVE_DOTS, ACTIVE_ROWS, DOTS_PER_LINE};
use v2c02_dots::vram;
use v2c02_fast::Fast;

fn main() {
    let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../goldens/p1-dots.bin");
    let golden = std::fs::read(golden_path).expect("goldens/p1-dots.bin (run first-light)");
    let mut f = Fast::new(vram);
    // The world's register program leaves t = 0 (the two $2005 writes
    // follow the $2006 pair and zero the scroll) and v = $2000 until the
    // pre-render copies; see docs/p3-plan.md.
    f.t = 0;
    f.v = 0x2000;
    let frame = f.frame();
    println!("palette as loaded: {}", f.palette.iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" "));

    let mut best = (usize::MAX, 0i32);
    for offset in 0i32..=6 {
        let mut bad = 0usize;
        for r in 0..ACTIVE_ROWS {
            for d in 1..=ACTIVE_DOTS {
                let gd = d as i32 + offset;
                let g = golden[r * DOTS_PER_LINE + gd as usize];
                if g != frame.at(r, d).0 {
                    bad += 1;
                }
            }
        }
        println!("golden dot = ours + {offset}: {bad} mismatching dots of {}", ACTIVE_ROWS * ACTIVE_DOTS);
        if bad < best.0 {
            best = (bad, offset);
        }
    }
    println!("best offset {} with {} mismatches", best.1, best.0);
    let mut shown = 0;
    for r in 0..ACTIVE_ROWS {
        for d in 1..=ACTIVE_DOTS {
            let g = golden[r * DOTS_PER_LINE + d + best.1 as usize];
            let c = frame.at(r, d).0;
            if g != c && shown < 16 {
                println!("  row {r} dot {d}: golden {g:02x} ours {c:02x}");
                shown += 1;
            }
        }
    }
}
