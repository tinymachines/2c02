//! The P3 step-1 gate: the per-dot stepper against rung 0's dot golden,
//! every visible dot, and against the frame period.
//!
//! SKIPs by name without the table (extern not fetched at build time) or
//! the golden (`goldens/p1-dots.bin`, written by `first-light`);
//! `REQUIRE_GOLDEN_P3=1` insists. `MUTATE=1` drops INC_X from the table
//! and must go red. The timing gate runs in release only.

use nes_bus::{ACTIVE_DOTS, ACTIVE_ROWS, DOTS_PER_LINE, LINES, SAMPLES_PER_DOT};
use ntsc_grid::SampleRate;
use v2c02_dots::{standard_program, vram};
use v2c02_fast::{table, palette_as_loaded, Fast, INC_X};

/// Where the golden holds pixel x: rung 0's `pal_d` presents pixel x at
/// hpos x + 3 (measured per half-step by `p3-pal-probe`: `pal_ptr` is
/// `pixel_color` one dot later, `pal_d` is the RAM at `pal_ptr`, and
/// `pixel_color` at hpos h is pixel h - 2), and P1's capture writes hpos
/// h to dot h + 1. So golden dot = stepper dot + 3. Pinned from the fit
/// in `examples/p3-fit.rs`, whose minimum this must be.
const GOLDEN_PIXEL_OFFSET: usize = 3;

fn golden() -> Option<Vec<u8>> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../goldens/p1-dots.bin");
    match std::fs::read(path) {
        Ok(g) if g.len() == LINES * DOTS_PER_LINE => Some(g),
        _ => {
            if std::env::var("REQUIRE_GOLDEN_P3").is_ok() {
                panic!("REQUIRE_GOLDEN_P3 set but goldens/p1-dots.bin is missing");
            }
            None
        }
    }
}

fn stepper() -> Fast {
    let mut t = table();
    if std::env::var("MUTATE").is_ok_and(|v| v == "1") {
        // Drop data, not a label: without the coarse X increment every
        // tile on a line is the same tile and the picture cannot match.
        t.iter_mut().for_each(|e| *e &= !INC_X);
    }
    let mut f = Fast::with_table(t, vram, palette_as_loaded());
    // The standard world as a register program through the register
    // file: its $2002 read, then the writes. That leaves t = 0 (the
    // $2005 pair follows the $2006 pair) and v = $2000 until the
    // pre-render copies; asserted, since the program is the claim.
    f.read(2);
    f.run_program(&standard_program());
    assert_eq!((f.t, f.v, f.fine_x), (0, 0x2000, 0), "the register file after the standard program");
    // The palette the register file derives from the program is the
    // palette the chip holds, read back by build.rs: the world writes
    // it paced and the engine lands it as written (halfphi 0.1.6).
    assert_eq!(f.palette, palette_as_loaded(), "the palette through the register file against the chip's read-back");
    f
}

#[test]
fn the_stepper_matches_rung_0_on_every_visible_dot() {
    if !v2c02_fast::table_available() {
        eprintln!("SKIP: no table (extern/visual2c02 not fetched at build time)");
        return;
    }
    let Some(golden) = golden() else {
        eprintln!("SKIP: no goldens/p1-dots.bin");
        return;
    };
    let mut f = stepper();
    let frame = f.frame();
    let backdrop = f.palette[0];
    let mut bad = Vec::new();
    for r in 0..ACTIVE_ROWS {
        // The three dots before pixel 0 present index 0 (measured:
        // `pixel_color` is 0 at hpos 0 and 1 on every row probed).
        for d in 1..=GOLDEN_PIXEL_OFFSET {
            let g = golden[r * DOTS_PER_LINE + d];
            if g != backdrop {
                bad.push((r, d, g, backdrop));
            }
        }
        for d in 1..=ACTIVE_DOTS {
            let g = golden[r * DOTS_PER_LINE + d + GOLDEN_PIXEL_OFFSET];
            let c = frame.at(r, d).0;
            if g != c {
                bad.push((r, d + GOLDEN_PIXEL_OFFSET, g, c));
            }
        }
    }
    if !bad.is_empty() {
        for (r, d, g, c) in bad.iter().take(12) {
            eprintln!("row {r} golden dot {d}: golden {g:02x} stepper {c:02x}");
        }
        panic!("{} of {} visible dots disagree with rung 0", bad.len(), ACTIVE_ROWS * (ACTIVE_DOTS + GOLDEN_PIXEL_OFFSET));
    }
    eprintln!(
        "{} visible dots agree with rung 0, backdrop {backdrop:02x}, palette as held {}",
        ACTIVE_ROWS * (ACTIVE_DOTS + GOLDEN_PIXEL_OFFSET),
        f.palette[..16].iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" ")
    );
}

#[test]
fn a_frame_renders_inside_the_frame_period() {
    if !v2c02_fast::table_available() {
        eprintln!("SKIP: no table (extern/visual2c02 not fetched at build time)");
        return;
    }
    if cfg!(debug_assertions) {
        eprintln!("SKIP: the timing gate is a release measurement (cargo test --release)");
        return;
    }
    // The frame period, derived: half-steps per frame over the grid
    // rate (12 x f_sc, f_sc = 315/88 MHz), never typed.
    let rate = SampleRate::grid().0;
    let frame_samples = (LINES * DOTS_PER_LINE * SAMPLES_PER_DOT) as f64;
    let period_s = frame_samples * (*rate.denom() as f64) / (*rate.numer() as f64);

    let mut f = stepper();
    let _ = f.frame(); // warm
    let n = 200;
    let mut worst = 0.0f64;
    let t0 = std::time::Instant::now();
    for _ in 0..n {
        let t = std::time::Instant::now();
        let fr = f.frame();
        let dt = t.elapsed().as_secs_f64();
        worst = worst.max(dt);
        std::hint::black_box(fr);
    }
    let mean = t0.elapsed().as_secs_f64() / n as f64;
    eprintln!(
        "frame period {:.3} ms; {n} frames: mean {:.3} ms ({:.1}x inside), worst {:.3} ms ({:.1}x inside)",
        period_s * 1e3,
        mean * 1e3,
        period_s / mean,
        worst * 1e3,
        period_s / worst
    );
    assert!(mean <= period_s, "mean frame {:.3} ms exceeds the period {:.3} ms", mean * 1e3, period_s * 1e3);
    assert!(worst <= period_s, "worst frame {:.3} ms exceeds the period {:.3} ms", worst * 1e3, period_s * 1e3);
}
