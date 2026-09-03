//! Parses the Visual 2C02 die data through halfphi and embeds the
//! resulting netlist blob, plus the counts measured from it (which the
//! tests hold to the handoff sketch's independently measured numbers).
//!
//! Without extern/visual2c02 (tools/fetch-netlist.sh) the crate still
//! builds, data-free: the blob is empty, `netlist_missing` is set, and
//! the library refuses at runtime by name. A fresh clone must build and
//! its tests must SKIP loudly rather than fail, the family pattern.

use std::path::Path;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(netlist_missing)");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let ext = Path::new(&manifest).join("../../extern/visual2c02");
    println!("cargo:rerun-if-changed={}", ext.display());
    let out = std::env::var("OUT_DIR").unwrap();
    let out = Path::new(&out);

    if !ext.join("segdefs.js").exists() {
        println!("cargo::rustc-cfg=netlist_missing");
        println!(
            "cargo:warning=extern/visual2c02 not fetched (tools/fetch-netlist.sh); building data-free"
        );
        std::fs::write(out.join("netlist.bin"), []).unwrap();
        std::fs::write(
            out.join("counts.rs"),
            "pub const NODE_COUNT: usize = 0;\npub const TRANSISTOR_COUNT: usize = 0;\npub const NAME_COUNT: usize = 0;\npub const RAIL_CONFLICT_HOLDS: &[u16] = &[];\n",
        )
        .unwrap();
        std::fs::write(out.join("areas.rs"), "pub const NODE_AREAS: &[f64] = &[];\n").unwrap();
        return;
    }

    let read = |f: &str| std::fs::read_to_string(ext.join(f)).unwrap();
    let parsed = halfphi::parse(&halfphi::ChipSource {
        segdefs: &read("segdefs.js"),
        transdefs: &read("transdefs.js"),
        nodenames: &read("nodenames.js"),
        // The 2C02 spells its rails gnd/pwr, a third spelling in the
        // family (6502 vss/vcc, 6800 gnd/vcc): the reason rails are a
        // parameter.
        rails: halfphi::Rails { ground: "gnd", supply: "pwr" },
    })
    .expect("visual2c02 data did not parse");
    let nl = halfphi::Netlist::decode(&parsed.blob).expect("blob decodes");

    // The rail-conflict hold list: the nodes the reference's own chipsim
    // special-cases when a group holds both rails (its getNodeValue
    // suppresses gnd and pwr for exactly these, the spr_d OAM data
    // lines). Extracted from the pinned chipsim.js rather than typed, so
    // the claim comes from the artifact making it; the counts test holds
    // the list to the spr_d names independently.
    let chipsim = read("chipsim.js");
    let mut holds: Vec<u32> = Vec::new();
    for chunk in chipsim.split("arrayContains(group, ").skip(1) {
        let digits: String = chunk.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            let id: u32 = digits.parse().unwrap();
            if !holds.contains(&id) {
                holds.push(id);
            }
        }
    }
    holds.sort_unstable();
    assert!(!holds.is_empty(), "chipsim.js no longer names its rail-conflict nodes");

    // Per-node areas for the hold's charge vote, computed the way the
    // reference's wires.js computes them: twice the shoelace area per
    // polygon, absolute, summed per node, with the rails accumulating
    // nothing (wires.js skips gnd's polygons entirely and skips pwr in
    // the area sum).
    let mut areas = vec![0.0f64; nl.node_count()];
    for poly in &parsed.polygons {
        let n = poly.node;
        if n == nl.vss() || n == nl.vcc() {
            continue;
        }
        let pts = &poly.pts;
        if pts.len() < 3 {
            continue;
        }
        let mut a = 0.0f64;
        for i in 0..pts.len() {
            let (x1, y1) = pts[i];
            let (x2, y2) = pts[(i + 1) % pts.len()];
            a += x1 as f64 * y2 as f64 - x2 as f64 * y1 as f64;
        }
        areas[n as usize] += a.abs();
    }
    let mut areas_src = String::from("pub const NODE_AREAS: &[f64] = &[\n");
    for chunk in areas.chunks(16) {
        areas_src.push_str("    ");
        for v in chunk {
            areas_src.push_str(&format!("{v:.1},"));
        }
        areas_src.push('\n');
    }
    areas_src.push_str("];\n");
    std::fs::write(out.join("areas.rs"), areas_src).unwrap();

    std::fs::write(out.join("netlist.bin"), &parsed.blob).unwrap();
    std::fs::write(
        out.join("counts.rs"),
        format!(
            "pub const NODE_COUNT: usize = {};\npub const TRANSISTOR_COUNT: usize = {};\npub const NAME_COUNT: usize = {};\npub const RAIL_CONFLICT_HOLDS: &[u16] = &{:?};\n",
            nl.node_count(),
            nl.transistor_count(),
            parsed.name_count,
            holds,
        ),
    )
    .unwrap();
}
