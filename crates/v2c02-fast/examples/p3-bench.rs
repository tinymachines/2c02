//! Frame time of the per-dot stepper, the number P3's plan asked for
//! before any slicing work: mean and worst over a stated number of
//! frames against the frame period (derived from the grid rate).

use nes_bus::{DOTS_PER_LINE, LINES, SAMPLES_PER_DOT};
use ntsc_grid::SampleRate;
use v2c02_dots::vram;
use v2c02_fast::Fast;

fn main() {
    let rate = SampleRate::grid().0;
    let period_s = (LINES * DOTS_PER_LINE * SAMPLES_PER_DOT) as f64 * (*rate.denom() as f64) / (*rate.numer() as f64);
    let mut f = Fast::new(vram);
    f.t = 0;
    f.v = 0x2000;
    let _ = f.frame();
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(500);
    let mut worst = 0.0f64;
    let t0 = std::time::Instant::now();
    for _ in 0..n {
        let t = std::time::Instant::now();
        std::hint::black_box(f.frame());
        worst = worst.max(t.elapsed().as_secs_f64());
    }
    let mean = t0.elapsed().as_secs_f64() / n as f64;
    println!(
        "frame period {:.3} ms; {n} frames: mean {:.3} ms ({:.1}x inside the period, {:.0} frames/s), worst {:.3} ms ({:.1}x inside)",
        period_s * 1e3,
        mean * 1e3,
        period_s / mean,
        1.0 / mean,
        worst * 1e3,
        period_s / worst
    );
}
