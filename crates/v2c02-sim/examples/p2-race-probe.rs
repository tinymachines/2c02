//! The VBL read race, measured: $2002 reads whose access lands at
//! chosen half-step offsets around the flag set (vpos 241, hpos 1,
//! measured by p2-probe), with NMI enabled, one alignment per frame.
//! For each alignment: the returned bit 7, the /INT level shortly
//! after, and whether a follow-up read later the same frame still sees
//! the flag. Rendering stays off, so every frame is the same length
//! and the probe self-calibrates from two observed sets.

use v2c02_sim::harness::Harness;
use v2c02_sim::Ppu;

fn vram(_a: u16) -> u8 {
    0
}

fn main() {
    let mut h = Harness::new(Ppu::power_on(), vram);
    h.wait(712_100);
    // NMI enabled, rendering off. The $2002 read first clears the
    // undefined write toggle, as P1 measured.
    for (rw, reg, val) in [(true, 2, 0x00), (false, 0, 0x80), (false, 1, 0x00)] {
        h.cpu_access(rw, reg, val);
    }

    let nl = h.ppu.engine.netlist().clone();
    let n = |name: &str| nl.node(name).unwrap_or_else(|| panic!("node {name}"));
    let set_vbl = n("set_vbl_flag");
    let int = n("int");

    // Self-calibrate: watch two rises of set_vbl_flag.
    let mut rises = Vec::new();
    let mut was = h.ppu.engine.is_high(set_vbl);
    while rises.len() < 2 {
        h.half_step();
        let s = h.ppu.engine.is_high(set_vbl);
        if s && !was {
            rises.push(h.half_steps);
        }
        was = s;
    }
    let period = rises[1] - rises[0];
    println!("set_vbl_flag at {} and {}, period {period}", rises[0], rises[1]);

    // One alignment per frame: the read access STARTS at
    // (next set + offset). Offsets chosen to sweep the access window
    // across the set moment.
    let offsets: [i64; 8] = [-30, -24, -18, -12, -6, 0, 6, 12];
    println!("offset  bit7  int_after  later_read_bit7");
    for (k, off) in offsets.iter().enumerate() {
        let target = (rises[1] as i64 + period as i64 * (k as i64 + 1) + off) as u64;
        while h.half_steps < target {
            h.half_step();
        }
        let status = h.read(2);
        let int_after = h.ppu.engine.is_high(int);
        h.wait(200);
        let later = h.read(2);
        println!(
            "{off:>6}  {}     {}          {}",
            (status >> 7) & 1,
            int_after as u8,
            (later >> 7) & 1
        );
        // Clear state for the next frame: the late read above already
        // consumed the flag if it was still set.
    }
}
