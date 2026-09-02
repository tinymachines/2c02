//! Rung-0 throughput, replacing the handoff sketch's authored estimate.
use std::time::Instant;
use v2c02_sim::Ppu;
fn main() {
    let mut ppu = Ppu::power_on();
    for _ in 0..100 { ppu.half_step(); } // warm
    let n = 2000;
    let t = Instant::now();
    for _ in 0..n { ppu.half_step(); }
    let dt = t.elapsed().as_secs_f64();
    let rate = n as f64 / dt;
    // A frame is 89,342 dots; the master clock is 4 per dot, two half
    // steps per master cycle.
    let per_frame = 89_342.0 * 8.0 / rate;
    println!("rung 0: {rate:.0} half-steps/s ({per_frame:.0} s per frame)");
}
