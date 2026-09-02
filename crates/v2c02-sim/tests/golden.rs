//! P0's gates: the counts held to the sketch's independent measurement,
//! power-on convergence, and the reference simulator's own trace
//! replayed node for node, half step for half step.
//!
//! SKIPS by name without the fetched die data (tools/fetch-netlist.sh)
//! or without the golden file (tools/golden-trace/gen.js);
//! REQUIRE_NETLIST=1 / REQUIRE_GOLDEN=1 make absence a failure. MUTATE=1
//! switches off the supply-gated transistor fix-up in the subject, the
//! one piece of chip knowledge power_on adds; the golden replay must
//! diverge, which is the proof those 38 conducting transistors are
//! load-bearing. (A first mutation floated io_ce instead, and the node
//! simply held its charge: a mutation the subject survives by design
//! proves nothing, the capture source's lesson verbatim.)

use v2c02_sim::Ppu;

fn skip(reason: &str, require_var: &str) -> bool {
    if std::env::var(require_var).map(|v| v == "1").unwrap_or(false) {
        panic!("{require_var}=1 but {reason}");
    }
    eprintln!("SKIP: {reason}");
    true
}

fn mutate() -> bool {
    std::env::var("MUTATE").map(|v| v == "1").unwrap_or(false)
}

/// The subject: the honest power-on, or under MUTATE=1 one whose 38
/// supply-gated transistors are switched back off, halfphi's default
/// and the wrong physics.
fn subject() -> Ppu {
    let mut ppu = Ppu::power_on();
    if mutate() {
        let vcc = ppu.engine.netlist().vcc();
        let gated: Vec<_> = ppu.engine.netlist().gates_of(vcc).to_vec();
        for t in gated {
            ppu.engine.state_mut().trans_on.clear(t as usize);
        }
        ppu.engine.settle_all();
    }
    ppu
}

#[test]
fn the_counts_match_the_sketchs_independent_measurement() {
    if !v2c02_netlist::available() && skip("extern/visual2c02 not fetched", "REQUIRE_NETLIST") {
        return;
    }
    // The sketch first said 16,871, measured by a regex that counted
    // the 114 transistors Quietust left inside a block comment; a
    // comment-aware regex then said 16,757, fumbling one fused entry.
    // The two REAL parsers, halfphi and the reference JS engine itself,
    // agree: 16,758 conducting transistors, 8,770 nodes. Instruments
    // lie before the code does; the number below is the parsers'.
    assert_eq!(v2c02_netlist::TRANSISTOR_COUNT, 16_758);
    let nl = v2c02_netlist::netlist();
    assert_eq!(nl.transistor_count(), v2c02_netlist::TRANSISTOR_COUNT);
    // The rails, and the two permanently-decided transistor families:
    // 38 supply-gated (conducting in silicon; power_on sets them so)
    // and 46 ground-gated (off in silicon and in the model).
    assert_eq!(nl.gates_of(nl.vcc()).len(), 38);
    assert_eq!(nl.gates_of(nl.vss()).len(), 46);
    for name in ["clk0", "res", "io_ce", "int", "vid_emph", "vid_burst_h", "vid_luma0_h"] {
        assert!(nl.node(name).is_some(), "node {name} missing");
    }
    eprintln!(
        "2c02: {} nodes, {} transistors, {} names",
        v2c02_netlist::NODE_COUNT,
        v2c02_netlist::TRANSISTOR_COUNT,
        v2c02_netlist::NAME_COUNT
    );
}

#[test]
fn power_on_converges_with_no_chip_specific_help() {
    if !v2c02_netlist::available() && skip("extern/visual2c02 not fetched", "REQUIRE_NETLIST") {
        return;
    }
    let ppu = Ppu::power_on();
    let cold = ppu.engine.stats().nonconvergent_settles;
    assert_eq!(cold, 0, "nonconvergent settles during power-on: {cold}");
}

#[test]
fn the_reference_trace_replays_node_for_node() {
    if !v2c02_netlist::available() && skip("extern/visual2c02 not fetched", "REQUIRE_NETLIST") {
        return;
    }
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/golden-trace/golden-2c02.txt"
    );
    let Ok(golden) = std::fs::read_to_string(path) else {
        if skip("no golden trace (node tools/golden-trace/gen.js)", "REQUIRE_GOLDEN") {
            return;
        }
        unreachable!()
    };
    let mut lines = golden.lines();
    let header = lines.next().expect("golden header");
    assert!(header.starts_with("2c02 golden:"), "not a 2c02 golden: {header}");

    let mut ppu = subject();
    // The measured exemption, closed by construction: nine sprite-path
    // input latches are dynamic storage with no reset connection, so
    // their power-on state is genuinely undefined; both engines are
    // deterministic about the coin and flip it differently (measured
    // 2026-09-02: six flush by half-step 14 once real values move
    // through them, three, the x-flip input latch and its followers,
    // are never written in a memory-less free-run and hold theirs for
    // the whole trace; every one of the other 10,897 nodes is
    // bit-exact across all 601 states). Two followers are unnamed and
    // exempted by id. The list is asserted CLOSED below because a
    // masked comparison is only as honest as its mask is small: the
    // 6502's rail-write bug hid behind exactly this shape of blindness.
    let nl = ppu.engine.netlist().clone();
    let named = [
        "x_flip_flag_in",
        "/x_flip_flag_in",
        "x_flip_flag_in_2",
        "spr_d6_in",
        "/(spr_d6_in_and_+sprite_in_range_reg)",
        "spr_d1_in",
        "/(spr_d1_in_and_+sprite_in_range_reg)",
    ];
    let persistent = ["x_flip_flag_in", "/x_flip_flag_in", "x_flip_flag_in_2"];
    let mut exempt_early: Vec<usize> = named
        .iter()
        .map(|n| nl.node(n).unwrap_or_else(|| panic!("exempt node {n} missing")) as usize)
        .collect();
    exempt_early.extend([10_712usize, 10_738]); // unnamed followers of the same latches
    let exempt_late: Vec<usize> = persistent
        .iter()
        .map(|n| nl.node(n).unwrap() as usize)
        .collect();
    const FLUSH_STEP: usize = 15;

    let mut compared = 0usize;
    for (step, want) in lines.enumerate() {
        if step > 0 {
            ppu.half_step();
        }
        let got = ppu.state_line();
        assert_eq!(got.len(), want.len(), "node count differs at step {step}");
        let allowed: &[usize] = if step < FLUSH_STEP { &exempt_early } else { &exempt_late };
        for (i, (a, b)) in got.bytes().zip(want.bytes()).enumerate() {
            if a != b && !allowed.contains(&i) {
                let name = nl.name_of(i as halfphi::NodeId).unwrap_or("(unnamed)");
                panic!(
                    "step {step}: divergence at node {i} ({name}) outside the closed exemption"
                );
            }
        }
        compared += 1;
    }
    assert!(compared > 100, "golden too short to mean anything: {compared}");
    eprintln!(
        "replayed {compared} states bit-exact outside {} undefined power-on latches",
        exempt_early.len()
    );
}
