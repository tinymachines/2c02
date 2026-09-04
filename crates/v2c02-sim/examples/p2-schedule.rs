//! The P2 dry run: execute the whole three-scenario program on the
//! proven engine and print the SCHEDULE as JSON: every register access
//! and every golden dump window as absolute half-step indices. The JS
//! generator (gen-p2.js) executes this schedule blindly, so the two
//! engines follow one trajectory with no counter-reading on the slow
//! side; the Rust tests replay the same schedule and compare inside
//! the windows.
//!
//! Scenario 1, sprite 0: the P1 register program, paced OAM writes
//! (y=90, tile=1, attr=0, x=180), rendering on, windows around the
//! fetch-line counter load and the display-line hit.
//! Scenario 2, the VBL race: NMI on, rendering off, three $2002 reads
//! in consecutive frames at the measured alignments (miss, suppress,
//! consume), a window around each.
//! Scenario 3, OAM corruption: identity refill, OAMADDR parked at $28,
//! one rendered frame, read-back, a window around the frame start.

use v2c02_sim::harness::Harness;
use v2c02_sim::Ppu;

fn vram(a: u16) -> u8 {
    let a = a & 0x3fff;
    match a {
        0x0010..=0x001f => 0xff,
        0x0000..=0x1fff => 0x00,
        _ => {
            let nt = a & 0x0fff;
            if nt & 0x03ff < 0x03c0 { 0x01 } else { 0x00 }
        }
    }
}

const PALETTE: [u8; 16] = [
    0x0f, 0x16, 0x2a, 0x12, 0x0f, 0x28, 0x14, 0x02,
    0x0f, 0x26, 0x1a, 0x31, 0x0f, 0x30, 0x27, 0x06,
];

struct Script {
    h: Harness,
    /// (half_step_at_start, rw, reg, val)
    accesses: Vec<(u64, bool, u8, u8)>,
    windows: Vec<(String, u64, u64)>,
}

impl Script {
    fn access(&mut self, rw: bool, reg: u8, val: u8) {
        self.accesses.push((self.h.half_steps, rw, reg, val));
        self.h.cpu_access(rw, reg, val);
    }
    fn run_to(&mut self, vpos: u32, hpos: u32) {
        let nl = self.h.ppu.engine.netlist().clone();
        let vp: Vec<_> = (0..9).map(|i| nl.node(&format!("vpos{i}")).unwrap()).collect();
        let hp: Vec<_> = (0..9).map(|i| nl.node(&format!("hpos{i}")).unwrap()).collect();
        let bits = |h: &Harness, ns: &[halfphi::NodeId]| -> u32 {
            ns.iter().enumerate().map(|(i, &n)| (h.ppu.engine.is_high(n) as u32) << i).sum()
        };
        while !(bits(&self.h, &vp) == vpos && bits(&self.h, &hp) >= hpos) {
            self.h.half_step();
        }
    }
    fn window(&mut self, name: &str, len: u64) {
        self.windows.push((name.into(), self.h.half_steps, self.h.half_steps + len));
        for _ in 0..len {
            self.h.half_step();
        }
    }
}

fn main() {
    let mut s = Script {
        h: Harness::new(Ppu::power_on(), vram),
        accesses: Vec::new(),
        windows: Vec::new(),
    };
    s.h.wait(712_100);

    // Scenario 1: the P1 program, sprite 0, rendering on.
    s.access(true, 2, 0x00);
    s.access(false, 0, 0x00);
    s.access(false, 1, 0x00);
    s.access(false, 6, 0x3f);
    s.access(false, 6, 0x00);
    for v in PALETTE {
        s.access(false, 7, v);
    }
    s.access(false, 6, 0x20);
    s.access(false, 6, 0x00);
    s.access(false, 5, 0x00);
    s.access(false, 5, 0x00);
    // ALL of OAM is initialized before rendering ever runs: the first
    // schedule left rows unwritten, their cells' power-on coins differed
    // between the engines, and once rendering returned in scenario 3
    // sprite evaluation read the coins and forked the whole render
    // pipeline (measured: 8 cells through the race windows, 248 nodes
    // by the corruption window). A golden over undefined storage is a
    // coin toss, so no storage is left undefined: y bytes at 0xF0 keep
    // every garbage sprite off-screen, then sprite 0's four bytes.
    s.access(false, 3, 0x00);
    for _ in 0..64u8 {
        for v in [0xf0u8, 0x00, 0x00, 0x00] {
            s.access(false, 4, v);
            s.h.wait(24);
        }
    }
    s.access(false, 3, 0x00);
    for v in [90u8, 1, 0, 180] {
        s.access(false, 4, v);
        s.h.wait(24);
    }
    s.access(false, 1, 0x1e);
    // The fetch-line load and the display-line hit.
    s.run_to(90, 290);
    s.window("sprite0-load", 200);
    s.run_to(91, 160);
    s.window("sprite0-hit", 400);

    // Scenario 2: the race. NMI on, rendering off.
    s.access(false, 1, 0x00);
    s.access(false, 0, 0x80);
    // Calibrate: find the next set (vpos 241 hpos 1), then schedule
    // reads in the following frames at the measured alignments
    // relative to that cadence (rendering off: period 714,736).
    s.run_to(241, 1);
    let set0 = s.h.half_steps;
    const PERIOD: u64 = 714_736;
    for (k, off) in [(-24i64, "miss"), (-12, "suppress"), (6, "consume")]
        .iter()
        .enumerate()
        .map(|(k, &(o, _))| (k as u64 + 1, o))
    {
        let target = (set0 as i64 + (PERIOD * k) as i64 + off) as u64;
        while s.h.half_steps < target - 40 {
            s.h.half_step();
        }
        s.windows.push((format!("race-{off}"), s.h.half_steps, target + 64));
        while s.h.half_steps < target {
            s.h.half_step();
        }
        s.access(true, 2, 0x00);
        while s.h.half_steps < target + 64 {
            s.h.half_step();
        }
    }

    // Scenario 3: corruption. Identity refill, OAMADDR parked, one
    // rendered frame, read-back.
    s.access(false, 3, 0x00);
    for i in 0..64u8 {
        s.access(false, 4, i);
        s.h.wait(24);
    }
    s.access(false, 3, 0x28);
    s.access(false, 1, 0x1e);
    s.run_to(261, 300);
    s.window("corrupt-frame-start", 800);
    s.run_to(245, 0);
    s.access(false, 1, 0x00);
    for i in 0..16u8 {
        s.access(false, 3, i);
        s.h.wait(24);
        s.access(true, 4, 0x00);
        s.h.wait(24);
    }
    let end = s.h.half_steps;

    // The schedule, as JSON.
    println!("{{");
    println!("  \"note\": \"Written by p2-schedule (the dry run on the proven engine); gen-p2.js and tests/p2.rs both execute it blindly.\",");
    println!("  \"end\": {end},");
    println!("  \"accesses\": [");
    for (i, (at, rw, reg, val)) in s.accesses.iter().enumerate() {
        let comma = if i + 1 == s.accesses.len() { "" } else { "," };
        println!("    [{at}, {}, {reg}, {val}]{comma}", *rw as u8);
    }
    println!("  ],");
    println!("  \"windows\": [");
    for (i, (name, a, b)) in s.windows.iter().enumerate() {
        let comma = if i + 1 == s.windows.len() { "" } else { "," };
        println!("    [\"{name}\", {a}, {b}]{comma}");
    }
    println!("  ]");
    println!("}}");
}
