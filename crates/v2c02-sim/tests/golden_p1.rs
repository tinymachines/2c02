//! The P1 node golden: the reference engine driven through the same
//! world (warm-up, the register program, the shared VRAM function,
//! rendering on) and compared node for node over the register writes
//! and the first three thousand rendering half-steps. This is the
//! oracle that covers the HARNESS as well as the chip: the 24-edge CPU
//! protocol and the CHR bus service are exercised on both sides.
//!
//! SKIPS by name without the golden (node tools/golden-trace/gen-p1.js,
//! which takes about half an hour of the reference's time);
//! REQUIRE_GOLDEN_P1=1 insists. MUTATE=1 serves the CHR bus a wrong
//! byte for one address; the replay must diverge.
//!
//! The same nine undefined power-on latches are exempt as in the P0
//! golden, minus the six that flush (this trace starts a frame in):
//! only the x-flip trio remains, and it is expected to LEAVE the list
//! when a milestone writes real sprite data through it.

use v2c02_sim::harness::Harness;
use v2c02_sim::Ppu;

fn mutate() -> bool {
    std::env::var("MUTATE").map(|v| v == "1").unwrap_or(false)
}

fn vram(a: u16) -> u8 {
    (((a >> 4) ^ a) & 0xff) as u8
}

fn vram_mutated(a: u16) -> u8 {
    // Every nametable byte off by one bit: whatever window the golden
    // covers, a fetch lands in it.
    if a >= 0x2000 {
        vram(a) ^ 1
    } else {
        vram(a)
    }
}

const PALETTE: [u8; 16] = [
    0x0f, 0x16, 0x2a, 0x12, 0x0f, 0x28, 0x14, 0x02,
    0x0f, 0x26, 0x1a, 0x31, 0x0f, 0x30, 0x27, 0x06,
];

#[test]
fn the_reference_replays_through_the_harness() {
    if !v2c02_netlist::available() {
        eprintln!("SKIP: extern/visual2c02 not fetched");
        return;
    }
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/golden-trace/golden-2c02-p1.txt"
    );
    let Ok(golden) = std::fs::read_to_string(path) else {
        if std::env::var("REQUIRE_GOLDEN_P1").map(|v| v == "1").unwrap_or(false) {
            panic!("REQUIRE_GOLDEN_P1=1 but the P1 golden is absent");
        }
        eprintln!("SKIP: no P1 golden (node tools/golden-trace/gen-p1.js)");
        return;
    };
    let mut lines = golden.lines();
    let header = lines.next().expect("header");
    assert!(header.starts_with("2c02 p1 golden:"), "not a P1 golden: {header}");

    let mut h = Harness::new(Ppu::power_on(), if mutate() { vram_mutated } else { vram });
    h.wait(712_100);
    let nl = h.ppu.engine.netlist().clone();
    let exempt: Vec<usize> = ["x_flip_flag_in", "/x_flip_flag_in", "x_flip_flag_in_2"]
        .iter()
        .map(|n| nl.node(n).unwrap() as usize)
        .collect();

    // The golden dumps one state per half-step from the first access
    // edge onward; replay the same accesses, comparing after every
    // half-step. The access loop mirrors cpu_access but compares
    // inside, so the protocol edges themselves are checked.
    let mut want = lines;
    let mut compared = 0usize;
    let check = |h: &Harness, want: &mut std::str::Lines, at: &str| {
        let expect = want.next().unwrap_or_else(|| panic!("golden ended at {at}"));
        let got = h.ppu.state_line();
        for (i, (a, b)) in got.bytes().zip(expect.bytes()).enumerate() {
            if a != b && !exempt.contains(&i) {
                panic!(
                    "{at}: divergence at node {i} ({})",
                    nl.name_of(i as halfphi::NodeId).unwrap_or("(unnamed)")
                );
            }
        }
    };

    let writes: Vec<(bool, u8, u8)> = {
        let mut v = vec![(true, 2u8, 0u8), (false, 0, 0x00), (false, 1, 0x00), (false, 6, 0x3f), (false, 6, 0x00)];
        v.extend(PALETTE.iter().map(|&p| (false, 7, p)));
        v.extend([(false, 6, 0x20), (false, 6, 0x00), (false, 5, 0x00), (false, 5, 0x00), (false, 1, 0x0a)]);
        v
    };
    for (k, (rw, reg, val)) in writes.iter().enumerate() {
        for counter in (1..=24u32).rev() {
            h.access_edge(*rw, *reg, *val, counter);
            h.half_step();
            check(&h, &mut want, &format!("access {k} edge {counter}"));
            compared += 1;
        }
        h.end_access();
    }
    for i in 0..3_000usize {
        h.half_step();
        check(&h, &mut want, &format!("render step {i}"));
        compared += 1;
    }
    eprintln!("replayed {compared} states through the harness");
}
