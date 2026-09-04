//! Replay the P2 schedule to a chosen compared-state index and diff
//! every node against the golden there: how wide is the divergence,
//! and whose names are in it.

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

fn main() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/golden-trace/p2-schedule.json"
    ))
    .unwrap();
    let acc_region = text.split("\"accesses\": [").nth(1).unwrap().split("\"windows\"").next().unwrap();
    let mut accesses: Vec<(u64, bool, u8, u8)> = Vec::new();
    for row in acc_region.split('[').skip(1) {
        let nums: Vec<u64> = row
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap())
            .collect();
        if nums.len() == 4 {
            accesses.push((nums[0], nums[1] == 1, nums[2] as u8, nums[3] as u8));
        }
    }
    let args: Vec<String> = std::env::args().collect();
    let target: u64 = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(2_086_860);
    let golden_index: usize = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(600);
    let sweep: u64 = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(1);

    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/golden-trace/golden-2c02-p2.txt"
    ))
    .unwrap();
    let mut h = Harness::new(Ppu::power_on(), vram);
    let mut ai = 0usize;
    while h.half_steps < target {
        if ai < accesses.len() && accesses[ai].0 == h.half_steps {
            let (_, rw, reg, val) = accesses[ai];
            h.cpu_access(rw, reg, val);
            ai += 1;
        } else {
            h.half_step();
        }
    }
    let nl = h.ppu.engine.netlist().clone();
    // Sweep: union every differing node over `sweep` consecutive
    // states starting at (target, golden_index).
    let golden_lines: Vec<&str> = golden.lines().skip(1).collect();
    let mut union: std::collections::BTreeSet<usize> = Default::default();
    for k in 0..sweep {
        if k > 0 {
            h.half_step();
        }
        let want = golden_lines[golden_index + k as usize];
        let got = h.ppu.state_line();
        for (i, (a, b)) in got.bytes().zip(want.bytes()).enumerate() {
            if a != b {
                union.insert(i);
            }
        }
    }
    println!("half-steps {target}..+{sweep}: {} nodes ever differ", union.len());
    for i in &union {
        println!("  {i}:{}", nl.name_of(*i as halfphi::NodeId).unwrap_or("?"));
    }
}
