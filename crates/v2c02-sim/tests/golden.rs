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
    // The rail-conflict hold list build.rs extracts from chipsim.js must
    // be exactly the spr_d OAM data lines, derived here by NAME from the
    // netlist, so the two sources cross-check and neither is typed.
    let mut sprd: Vec<halfphi::NodeId> = (0..8)
        .map(|i| nl.node(&format!("spr_d{i}")).unwrap_or_else(|| panic!("spr_d{i}")))
        .collect();
    sprd.sort_unstable();
    assert_eq!(
        v2c02_netlist::RAIL_CONFLICT_HOLDS,
        &sprd[..],
        "chipsim.js's rail-conflict ids are not the spr_d bus"
    );
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
    // No exemption. P0 as first recorded (2026-09-02) masked nine
    // sprite-path input latches, read as dynamic storage whose power-on
    // state silicon leaves undefined and the two engines flip
    // differently. Under halfphi 0.1.6, which resolves an undriven group
    // by the reference's own area vote, every one of them agrees from
    // state 0 (2026-09-04): the "coin" was the engine's charge rule,
    // not the silicon. A masked comparison is only as honest as its
    // mask is small, and the smallest mask is none.
    let nl = ppu.engine.netlist().clone();
    let mut compared = 0usize;
    for (step, want) in lines.enumerate() {
        if step > 0 {
            ppu.half_step();
        }
        let got = ppu.state_line();
        assert_eq!(got.len(), want.len(), "node count differs at step {step}");
        for (i, (a, b)) in got.bytes().zip(want.bytes()).enumerate() {
            if a != b {
                let name = nl.name_of(i as halfphi::NodeId).unwrap_or("(unnamed)");
                panic!("step {step}: divergence at node {i} ({name})");
            }
        }
        compared += 1;
    }
    assert!(compared > 100, "golden too short to mean anything: {compared}");
    eprintln!("replayed {compared} states bit-exact on every node, no exemption");
}
