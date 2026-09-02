//! Measure, before authoring: replay the P1 golden through the harness
//! and report every node that EVER diverges, with its first and last
//! divergent state and how many states it disagrees in. The exemption
//! in tests/golden_p1.rs is written from this output, not guessed.

use v2c02_sim::harness::Harness;
use v2c02_sim::Ppu;

fn vram(a: u16) -> u8 {
    (((a >> 4) ^ a) & 0xff) as u8
}

const PALETTE: [u8; 16] = [
    0x0f, 0x16, 0x2a, 0x12, 0x0f, 0x28, 0x14, 0x02,
    0x0f, 0x26, 0x1a, 0x31, 0x0f, 0x30, 0x27, 0x06,
];

fn main() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/golden-trace/golden-2c02-p1.txt"
    );
    let golden = std::fs::read_to_string(path).expect("P1 golden");
    let mut lines = golden.lines();
    let header = lines.next().expect("header");
    assert!(header.starts_with("2c02 p1 golden:"), "not a P1 golden: {header}");

    let mut h = Harness::new(Ppu::power_on(), vram);
    h.wait(712_100);
    let nl = h.ppu.engine.netlist().clone();

    // (first, last, count) per node id that ever diverges.
    let mut diverging: std::collections::BTreeMap<usize, (usize, usize, usize)> =
        std::collections::BTreeMap::new();
    let mut state = 0usize;
    let mut check = |h: &Harness, want: &str| {
        let got = h.ppu.state_line();
        for (i, (a, b)) in got.bytes().zip(want.bytes()).enumerate() {
            if a != b {
                let e = diverging.entry(i).or_insert((state, state, 0));
                e.1 = state;
                e.2 += 1;
            }
        }
        state += 1;
    };

    let writes: Vec<(bool, u8, u8)> = {
        let mut v = vec![
            (true, 2u8, 0u8),
            (false, 0, 0x00),
            (false, 1, 0x00),
            (false, 6, 0x3f),
            (false, 6, 0x00),
        ];
        v.extend(PALETTE.iter().map(|&p| (false, 7, p)));
        v.extend([
            (false, 6, 0x20),
            (false, 6, 0x00),
            (false, 5, 0x00),
            (false, 5, 0x00),
            (false, 1, 0x0a),
        ]);
        v
    };
    let mut want = lines;
    for (rw, reg, val) in writes {
        for counter in (1..=24u32).rev() {
            h.access_edge(rw, reg, val, counter);
            h.half_step();
            check(&h, want.next().expect("golden ended in accesses"));
        }
        h.end_access();
    }
    for _ in 0..3_000usize {
        h.half_step();
        check(&h, want.next().expect("golden ended in render"));
    }

    println!("states compared: {state}");
    println!("nodes that ever diverge: {}", diverging.len());
    for (id, (first, last, count)) in &diverging {
        println!(
            "node {id:5} ({:40}) first {first:4} last {last:4} states {count}",
            nl.name_of(*id as halfphi::NodeId).unwrap_or("(unnamed)")
        );
    }
}
