//! The blank picture: what the stepper shows with rendering off, held
//! to rung 0's capture of the blank world (`v2c02-dots`'s
//! `blank-probe`: the standard program, mask $00, then `BLANK_WRITES`
//! on rows 60..66). Every visible dot of the captured rows must agree:
//! the backdrop where v is outside palette RAM, the entry v addresses
//! inside it, a $2006 pair showing in the dot it lands, a $2007 write's
//! step showing `BLANK_2007_HOLD` dots later. SKIPs by name without the
//! table or `goldens/blank.bin`; `REQUIRE_GOLDEN_P3=1` insists.
//! `MUTATE=1` shows the backdrop whatever v is and must go red;
//! `MUTATE=2` moves the $2007 hold by one dot either way and both must
//! go red.

use nes_bus::{ACTIVE_DOTS, DOTS_PER_LINE};
use v2c02_dots::{standard_program, vram, BLANK_ROWS, BLANK_WRITES};
use v2c02_fast::{table, DotWrite, Fast, BLANK_2007_HOLD};

/// The golden holds pixel x at dot x + 3 (see tests/p3.rs).
const GOLDEN_PIXEL_OFFSET: usize = 3;
/// The stepper applies a write two dots after its access starts
/// (tests/p3_scroll.rs, fitted).
const WRITE_DELAY: usize = 2;

fn golden() -> Option<([u8; 32], Vec<u8>)> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../goldens/blank.bin");
    match std::fs::read(path) {
        Ok(g) if g.len() == 32 + BLANK_ROWS * DOTS_PER_LINE => {
            let mut palette = [0u8; 32];
            palette.copy_from_slice(&g[..32]);
            Some((palette, g[32..].to_vec()))
        }
        _ => {
            if std::env::var("REQUIRE_GOLDEN_P3").is_ok() {
                panic!("REQUIRE_GOLDEN_P3 set but goldens/blank.bin is missing");
            }
            None
        }
    }
}

fn writes_at(delay: usize) -> Vec<DotWrite> {
    BLANK_WRITES
        .iter()
        .map(|w| DotWrite { vpos: w.vpos as usize, hpos: w.hpos as usize + delay, reg: w.reg, val: w.val })
        .collect()
}

fn mismatches(frame: &nes_bus::DotFrame, golden: &[u8]) -> Vec<(usize, usize, u8, u8)> {
    let mut bad = Vec::new();
    for r in 0..BLANK_ROWS {
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

#[test]
fn the_blank_picture_follows_v_into_palette_ram_as_the_chip_shows_it() {
    let Some((palette, golden)) = golden() else {
        eprintln!("SKIP: goldens/blank.bin missing (cargo run --release -p v2c02-dots --example blank-probe)");
        return;
    };
    let mutate = std::env::var("MUTATE").ok();
    // The palette as the chip holds it: the sixteen entries the world
    // wrote are what the register file derives (asserted); the rest are
    // power-on contents the picture walks through, taken from the chip.
    let stepper = || {
        let mut f = Fast::with_table(table(), vram, palette);
        f.read(2);
        f.run_program(&standard_program());
        assert_eq!(f.palette, palette, "the palette through the register file against the chip's read-back");
        f.write(1, 0x00);
        f
    };
    let mut f = stepper();
    if mutate.as_deref() == Some("1") {
        f.blank_shows_backdrop_only = true;
    }
    let mut writes = writes_at(WRITE_DELAY);
    if mutate.as_deref() == Some("2") {
        // The $2007 hold one dot either way: the writes' own delay
        // moves the $2007 accesses only, which is the hold's dot.
        for shift in [-1isize, 1] {
            let mut w = writes.clone();
            for x in w.iter_mut().filter(|x| x.reg == 7) {
                x.hpos = (x.hpos as isize + shift) as usize;
            }
            let mut f = stepper();
            let frame = f.frame_with_writes(&w);
            let bad = mismatches(&frame, &golden);
            assert!(!bad.is_empty(), "the $2007 hold moved by {shift} and the picture could not tell");
            eprintln!("$2007 hold {} + ({shift}): {} dots disagree", BLANK_2007_HOLD, bad.len());
        }
        panic!("MUTATE=2: both neighbours of the $2007 hold are red (this panic is the red)");
    }
    writes.sort_by_key(|w| (w.vpos, w.hpos));
    let frame = f.frame_with_writes(&writes);
    let bad = mismatches(&frame, &golden);
    for (r, x, g, c) in bad.iter().take(12) {
        eprintln!("row {r} pixel {x}: golden {g:02x} stepper {c:02x}");
    }
    assert!(bad.is_empty(), "{} of {} blank dots disagree with rung 0", bad.len(), BLANK_ROWS * ACTIVE_DOTS);
    // The emphasis bits ride beside the colour: row 67 raises red from
    // the dot its $2001 write lands to the dot the clearing write lands
    // (authored; the measured lead is in the stepper's note).
    for d in 1..=ACTIVE_DOTS {
        let e = frame.at(67, d).1;
        let want = if (102..202).contains(&d) { 1 } else { 0 };
        assert_eq!(e, want, "row 67 dot {d}: emphasis {e}, the $2001 writes land at 102 and 202");
    }
    assert!((0..67).all(|r| (1..=ACTIVE_DOTS).all(|d| frame.at(r, d).1 == 0)), "no emphasis before row 67");
    // The gate saw something: the captured rows carry more than the backdrop.
    let colours: std::collections::BTreeSet<u8> = (0..BLANK_ROWS).flat_map(|r| (1..=ACTIVE_DOTS).map(move |d| (r, d))).map(|(r, d)| frame.at(r, d).0).collect();
    assert!(colours.len() >= 8, "the blank world shows {} colours; the capture must carry the palette walk", colours.len());
    eprintln!("{} blank dots agree with rung 0 over {} colours; $2007 hold {BLANK_2007_HOLD} dots", BLANK_ROWS * ACTIVE_DOTS, colours.len());
}
