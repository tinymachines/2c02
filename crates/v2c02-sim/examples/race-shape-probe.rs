//! The $2002 race under the console's access shape, measured at every
//! half-step across the flag's set (vpos 241, hpos 1) and its clear
//! (the pre-render line), on the switch-level chip.
//!
//!     cargo run --release -p v2c02-sim --example race-shape-probe -- [set|clear] [console|cs6|reference]
//!
//! P2's race table (`p2-race-probe`) used the reference's 24-edge
//! protocol: address at edge 24, chip enable eight half-steps later,
//! the byte sampled and the enable released at edge 1, two CPU cycles
//! per access. On the NES-001 the PPU's chip select is the 74LS139's
//! decode of the address alone (nes-glue, held to the schematic), so it
//! falls with the address at the CPU's phi1, holds for the cycle's
//! twelve master half-steps, and the CPU samples the byte at the end of
//! phi2. That is `console`: address, R/W and /CS together at t, the
//! byte at t+11, release at t+12. `cs6` keeps the address at t and
//! drops /CS at t+6 (an M2-qualified select, which the board does not
//! wire, kept as the control). `reference` is P2's shape at the same
//! start times. Each offset runs from one saved chip state sixty
//! half-steps before the event, so the sweep is minutes, not hours.
//! Rendering off, NMI enabled; the flag is left clear before the set
//! sweep and set before the clear sweep.

use std::sync::Arc;
use halfphi::Engine;
use v2c02_sim::harness::Harness;
use v2c02_sim::{Ppu, Sig};

fn vram(_a: u16) -> u8 {
    0
}

fn ppu_from(nl: &Arc<halfphi::Netlist>, state: &halfphi::ChipState) -> Ppu {
    let mut engine = Engine::new(nl.clone());
    *engine.state_mut() = state.clone();
    let n = |name: &str| nl.node(name).unwrap_or_else(|| panic!("node {name}"));
    Ppu { engine, sig: Sig { clk0: n("clk0"), res: n("res"), io_ce: n("io_ce"), int: n("int") } }
}

/// One read of $2002 in the named shape, starting now; returns the
/// byte the CPU would sample.
fn read_shaped(h: &mut Harness, shape: &str) -> u8 {
    match shape {
        "reference" => h.read(2),
        _ => {
            let cs_at = if shape == "cs6" { 6 } else { 0 };
            let mut sampled = 0;
            let nl = h.ppu.engine.netlist_arc().clone();
            let (vbl, int) = (nl.node("vbl_flag").unwrap(), nl.node("int").unwrap());
            let mut last = (h.ppu.engine.is_high(vbl), h.ppu.engine.is_high(int));
            for i in 0..12u32 {
                if i == 0 {
                    h.access_edge(true, 2, 0, 24);
                }
                if i == cs_at {
                    h.access_edge(true, 2, 0, 16);
                }
                if i == 11 {
                    sampled = h.access_edge(true, 2, 0, 1).unwrap();
                }
                h.half_step();
                let now = (h.ppu.engine.is_high(vbl), h.ppu.engine.is_high(int));
                if now != last && std::env::var_os("LOG").is_some() {
                    println!("  in-read: vbl_flag {} int {} at read start {:+}", now.0 as u8, now.1 as u8, i as i64 + 1);
                    last = now;
                }
            }
            h.end_access();
            sampled
        }
    }
}

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "set".into());
    let shape = std::env::args().nth(2).unwrap_or_else(|| "console".into());
    let mut h = Harness::new(Ppu::power_on(), vram);
    h.wait(712_100);
    for (rw, reg, val) in [(true, 2, 0x00), (false, 0, 0x80), (false, 1, 0x00)] {
        h.cpu_access(rw, reg, val);
    }
    let nl = h.ppu.engine.netlist_arc().clone();
    let n = |name: &str| nl.node(name).unwrap_or_else(|| panic!("node {name}"));
    let event = n(if which == "set" { "set_vbl_flag" } else { "vbl_clear_flags" });
    let int = n("int");
    let vbl = n("vbl_flag");

    // Two rises of the event give its period; the state is saved sixty
    // half-steps before the third.
    let mut rises = Vec::new();
    let mut was = h.ppu.engine.is_high(event);
    while rises.len() < 2 {
        h.half_step();
        let s = h.ppu.engine.is_high(event);
        if s && !was {
            rises.push(h.half_steps);
        }
        was = s;
        if which == "set" && rises.len() == 1 && h.half_steps == rises[0] + 400 {
            // Consume the flag the first set left, so the sweep starts clear.
            h.read(2);
        }
    }
    let period = rises[1] - rises[0];
    if which == "set" {
        h.wait(400);
        h.read(2);
    }
    let third = rises[1] + period;
    const LEAD: u64 = 60;
    while h.half_steps < third - LEAD {
        h.half_step();
    }
    let saved = h.ppu.engine.state().clone();
    println!("{which}: {} at {} and {}, period {period}; shape {shape}; flag before the window: {}",
        if which == "set" { "set_vbl_flag" } else { "vbl_clear_flags" }, rises[0], rises[1],
        h.ppu.engine.is_high(vbl) as u8);
    // The pin's own timing, without a read: every transition of the
    // event, the flag and /INT from the saved state through the event.
    {
        let mut w = Harness::new(ppu_from(&nl, &saved), vram);
        let hpos0 = n("hpos0");
        let watch = [("event", event), ("vbl_flag", vbl), ("int", int), ("hpos0", hpos0)];
        let mut last: Vec<bool> = watch.iter().map(|(_, n)| w.ppu.engine.is_high(*n)).collect();
        for k in 0..(LEAD + 40) {
            w.half_step();
            for (i, (name, n)) in watch.iter().enumerate() {
                let v = w.ppu.engine.is_high(*n);
                if v != last[i] {
                    println!("no read: {name} -> {} at {:+}", v as u8, k as i64 + 1 - LEAD as i64);
                    last[i] = v;
                }
            }
        }
    }
    let log_reads: Vec<i64> = std::env::var("LOG").map(|v| v.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or_default();
    println!("offset  bit7  flag_after  nmi");
    for off in -48i64..=24 {
        let mut w = Harness::new(ppu_from(&nl, &saved), vram);
        let start = (LEAD as i64 + off) as u64;
        // /INT is watched at every half-step from the saved state to
        // forty past the read: a consuming read releases the level, so
        // "did NMI fire" is whether the line was ever low, which is what
        // the CPU's edge latch would have seen.
        let mut int_low = false;
        for _ in 0..start {
            w.half_step();
            int_low |= !w.ppu.engine.is_high(int);
        }
        let logging = log_reads.contains(&off);
        let mut last = (w.ppu.engine.is_high(vbl), w.ppu.engine.is_high(int));
        let byte = read_shaped(&mut w, &shape);
        int_low |= !w.ppu.engine.is_high(int);
        for k in 0..40 {
            w.half_step();
            let now = (w.ppu.engine.is_high(vbl), w.ppu.engine.is_high(int));
            if logging && now != last {
                println!("  read at {off:+}: vbl_flag {} int {} at read start {:+}", now.0 as u8, now.1 as u8, k as i64 + 13);
                last = now;
            }
            int_low |= !w.ppu.engine.is_high(int);
        }
        let flag_after = w.ppu.engine.is_high(vbl);
        println!("{off:>6}  {}     {}           {}", (byte >> 7) & 1, flag_after as u8, if int_low { "N" } else { "-" });
    }
}
