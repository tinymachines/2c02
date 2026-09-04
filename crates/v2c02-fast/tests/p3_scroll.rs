//! The P3 step-3 gate: the register file and scroll. The stepper is
//! given the scroll world as a register program (the standard world's,
//! then the scroll's) and its mid-frame writes at their dots, and must
//! match rung 0's frame on every visible dot, with the palette RAM read
//! back out of the chip (the open write-path question, docs/p3-report.md)
//! and everything else derived through the register file. SKIPs by name
//! without the table or `goldens/p3-scroll.bin`; `REQUIRE_GOLDEN_P3=1`
//! insists. `MUTATE=1` drops the horizontal copy from the table and must
//! go red.

use nes_bus::{ACTIVE_DOTS, ACTIVE_ROWS, DOTS_PER_LINE, LINES};
use v2c02_dots::{standard_program, vram, SCROLL_PROGRAM, SCROLL_WRITES};
use v2c02_fast::{table, DotWrite, Fast, COPY_X};

/// The golden holds pixel x at dot x + 3 (see tests/p3.rs).
const GOLDEN_PIXEL_OFFSET: usize = 3;

/// Dots after the start of a mid-frame write's access at which the
/// stepper applies it. FITTED against the scroll golden: the scan below
/// finds zero mismatches on a plateau exactly one access wide (24
/// half-steps, three dots) and one dot wrong on either side, so the
/// effect lands inside the access and the picture cannot tell which
/// dot. The centre is pinned; the plateau is asserted as measured.
const WRITE_DELAY: usize = 2;
const WRITE_DELAY_PLATEAU: [usize; 3] = [1, 2, 3];

fn golden() -> Option<([u8; 32], Vec<u8>)> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../goldens/p3-scroll.bin");
    match std::fs::read(path) {
        Ok(g) if g.len() == 32 + LINES * DOTS_PER_LINE => {
            let mut palette = [0u8; 32];
            palette.copy_from_slice(&g[..32]);
            Some((palette, g[32..].to_vec()))
        }
        _ => {
            if std::env::var("REQUIRE_GOLDEN_P3").is_ok() {
                panic!("REQUIRE_GOLDEN_P3 set but goldens/p3-scroll.bin is missing");
            }
            None
        }
    }
}

fn stepper(palette: [u8; 32]) -> Fast {
    let mut t = table();
    if std::env::var("MUTATE").is_ok_and(|v| v == "1") {
        t.iter_mut().for_each(|e| *e &= !COPY_X);
    }
    let mut f = Fast::with_table(t, vram, palette);
    // The scroll world's program, as the chip received it: the standard
    // world's after its $2002 read, rendering off, the scroll's.
    f.read(2);
    f.run_program(&standard_program());
    f.write(1, 0x00);
    f.run_program(&SCROLL_PROGRAM);
    // The palette the register file derives is the palette the chip
    // holds (paced writes, halfphi 0.1.6).
    assert_eq!(f.palette, palette, "the palette through the register file against the chip's read-back");
    f
}

fn mismatches(frame: &nes_bus::DotFrame, golden: &[u8]) -> Vec<(usize, usize, u8, u8)> {
    let mut bad = Vec::new();
    for r in 0..ACTIVE_ROWS {
        for d in 1..=ACTIVE_DOTS {
            let g = golden[r * DOTS_PER_LINE + d + GOLDEN_PIXEL_OFFSET];
            let c = frame.at(r, d).0;
            if g != c {
                bad.push((r, d - 1, g, c));
            }
        }
    }
    bad
}

fn writes_at(delay: usize) -> Vec<DotWrite> {
    SCROLL_WRITES
        .iter()
        .map(|w| DotWrite { vpos: w.vpos as usize, hpos: w.hpos as usize + delay, reg: w.reg, val: w.val })
        .collect()
}

#[test]
fn the_register_file_and_scroll_match_rung_0_on_every_visible_dot() {
    if !v2c02_fast::table_available() {
        eprintln!("SKIP: no table (extern/visual2c02 not fetched at build time)");
        return;
    }
    let Some((palette, golden)) = golden() else {
        eprintln!("SKIP: no goldens/p3-scroll.bin");
        return;
    };
    // The fit, run every time so the pinned delay is re-derived rather
    // than trusted: the delay with zero mismatches must be WRITE_DELAY
    // and no other.
    let mut zero_at = Vec::new();
    for delay in 0..=8 {
        let mut f = stepper(palette);
        let frame = f.frame_with_writes(&writes_at(delay));
        let bad = mismatches(&frame, &golden);
        eprintln!("write delay {delay}: {} mismatching dots", bad.len());
        if bad.is_empty() {
            zero_at.push(delay);
        }
    }
    let mut f = stepper(palette);
    let frame = f.frame_with_writes(&writes_at(WRITE_DELAY));
    let bad = mismatches(&frame, &golden);
    for (r, x, g, c) in bad.iter().take(12) {
        eprintln!("row {r} pixel {x}: golden {g:02x} stepper {c:02x}");
    }
    assert!(bad.is_empty(), "{} of {} visible dots disagree with rung 0 at the pinned write delay", bad.len(), ACTIVE_ROWS * ACTIVE_DOTS);
    assert_eq!(zero_at, WRITE_DELAY_PLATEAU.to_vec(), "the write-delay plateau as measured");
    eprintln!(
        "{} visible dots agree with rung 0 through the register file; v after the frame {:04x}, t {:04x}, fine x {}",
        ACTIVE_ROWS * ACTIVE_DOTS,
        f.v,
        f.t,
        f.fine_x
    );
}
