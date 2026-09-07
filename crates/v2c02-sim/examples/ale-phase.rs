//! ALE against the dot clock, in master half-steps: the bench's
//! alignment classifier (nes-bench/tools/b2-align.py) reads ALE off the
//! scope, and the console's `Alignment` is defined on pclk0 (ppu_phase
//! is the half-step of a dot's start). With rendering on, this prints
//! the offset of every ALE rise from the last pclk0 rise, over a stretch
//! of a rendered frame, so the offset is measured and not assumed.
//!
//!   cargo run --release -p v2c02-sim --example ale-phase
//!
//! Measured 2026-09-07: 2,508 rises over 40,000 half-steps, every one
//! on the half-step pclk0 rose (offset 0), sixteen half-steps apart
//! with the sprite fetches' eights.
use v2c02_sim::harness::Harness;
use v2c02_sim::Ppu;

fn vram(a: u16) -> u8 {
    (a as u8).wrapping_mul(7)
}

fn main() {
    let mut h = Harness::new(Ppu::power_on(), vram);
    // Past the power-on quiet, then rendering on (background and
    // sprites, no clipping), then into the next frame's picture.
    h.wait(712_100);
    h.write(0, 0x00);
    h.write(1, 0x1e);
    h.wait(720_000);
    let nl = h.ppu.engine.netlist().clone();
    let (pclk0, ale) = (nl.node("pclk0").unwrap(), nl.node("ale").unwrap());
    let (mut p, mut a) = (h.ppu.engine.is_high(pclk0), h.ppu.engine.is_high(ale));
    let mut last_rise = None;
    let mut offsets = std::collections::BTreeMap::new();
    let mut rises = 0u32;
    let mut sample = Vec::new();
    for m in 0..40_000u64 {
        h.half_step();
        let (np, na) = (h.ppu.engine.is_high(pclk0), h.ppu.engine.is_high(ale));
        if np && !p {
            last_rise = Some(m);
        }
        if na && !a {
            rises += 1;
            if let Some(r) = last_rise {
                *offsets.entry(m - r).or_insert(0u32) += 1;
                if sample.len() < 6 {
                    sample.push((m, r));
                }
            }
        }
        p = np;
        a = na;
    }
    println!("{rises} ALE rises over 40,000 half-steps with rendering on; ALE rise minus the last pclk0 rise, half-steps: {offsets:?}; first few (ale, pclk0): {sample:?}");
}
