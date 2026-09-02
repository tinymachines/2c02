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
            "pub const NODE_COUNT: usize = 0;\npub const TRANSISTOR_COUNT: usize = 0;\npub const NAME_COUNT: usize = 0;\n",
        )
        .unwrap();
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

    std::fs::write(out.join("netlist.bin"), &parsed.blob).unwrap();
    std::fs::write(
        out.join("counts.rs"),
        format!(
            "pub const NODE_COUNT: usize = {};\npub const TRANSISTOR_COUNT: usize = {};\npub const NAME_COUNT: usize = {};\n",
            nl.node_count(),
            nl.transistor_count(),
            parsed.name_count,
        ),
    )
    .unwrap();
}
