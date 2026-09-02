//! Rendering-on throughput, the number P0's report deferred to P1:
//! build the standard world (untimed: the 712,100-step warm-up is a
//! rendering-off measurement the P0 bench already makes), then time a
//! free run with background rendering enabled.

use std::time::Instant;
use v2c02_dots::standard_world;

fn main() {
    let mut h = standard_world();
    let n = 50_000u64;
    let t = Instant::now();
    h.wait(n);
    let dt = t.elapsed().as_secs_f64();
    println!(
        "rendering on: {n} half-steps in {dt:.2}s = {:.0} half-steps/s",
        n as f64 / dt
    );
}
