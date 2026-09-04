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
//! byte for one address; MUTATE=rd routes the CHR bus through the
//! nes-bus contract's mutated_rd_for_proof, flipping /RD's polarity at
//! the pin frame (the N0 contract gate). Either way the replay must
//! diverge.
//!
//! No exemption. The first recording (2026-09-02) masked a 27-node
//! family of sprite-path latches through state 1,981, read as undefined
//! power-on state circulating until real sprite data flushed it. The
//! re-recording (2026-09-04, the palette writes paced and halfphi 0.1.6
//! resolving undriven groups by the reference's area vote) replays
//! bit-exact on every node from the first state: the family was the
//! charge rule deciding floating groups differently from the reference,
//! not silicon leaving them undefined (examples/p1-diverge-probe.rs
//! reports zero nodes that ever diverge). Any divergent node fails.

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
    if std::env::var("MUTATE").as_deref() == Ok("rd") {
        h.mutate_rd_for_proof = true;
    }
    h.wait(712_100);
    let nl = h.ppu.engine.netlist().clone();

    // The golden dumps one state per half-step from the first access
    // edge onward; replay the same accesses, comparing after every
    // half-step on every node. The access loop mirrors cpu_access but
    // compares inside, so the protocol edges themselves are checked.
    let mut want = lines;
    let mut compared = 0usize;
    let check = |h: &Harness, want: &mut std::str::Lines, state: usize, at: &str| {
        let expect = want.next().unwrap_or_else(|| panic!("golden ended at {at}"));
        let got = h.ppu.state_line();
        for (i, (a, b)) in got.bytes().zip(expect.bytes()).enumerate() {
            if a != b {
                panic!(
                    "{at} (state {state}): divergence at node {i} ({})",
                    nl.name_of(i as halfphi::NodeId).unwrap_or("(unnamed)")
                );
            }
        }
    };

    // (read, register, value, idle after): the generator's WRITES. Each
    // $2007 palette write is followed by an access width of idle, so
    // the palette lands as written (written back to back, both engines
    // lose the first value; docs/p3-report.md). The idle is dumped and
    // compared like the access edges.
    let writes: Vec<(bool, u8, u8, u64)> = {
        let mut v = vec![(true, 2u8, 0u8, 0u64), (false, 0, 0x00, 0), (false, 1, 0x00, 0), (false, 6, 0x3f, 0), (false, 6, 0x00, 0)];
        v.extend(PALETTE.iter().map(|&p| (false, 7, p, 24)));
        v.extend([(false, 6, 0x20, 0), (false, 6, 0x00, 0), (false, 5, 0x00, 0), (false, 5, 0x00, 0), (false, 1, 0x0a, 0)]);
        v
    };
    for (k, (rw, reg, val, idle)) in writes.iter().enumerate() {
        for counter in (1..=24u32).rev() {
            h.access_edge(*rw, *reg, *val, counter);
            h.half_step();
            check(&h, &mut want, compared, &format!("access {k} edge {counter}"));
            compared += 1;
        }
        h.end_access();
        for i in 0..*idle {
            h.half_step();
            check(&h, &mut want, compared, &format!("access {k} idle {i}"));
            compared += 1;
        }
    }
    for i in 0..3_000usize {
        h.half_step();
        check(&h, &mut want, compared, &format!("render step {i}"));
        compared += 1;
    }
    assert!(want.next().is_none(), "the golden has more states than the replay");
    assert!(compared >= 4_000, "too few states compared: {compared}");
    eprintln!("replayed {compared} states through the harness, every node, no exemption");
}
