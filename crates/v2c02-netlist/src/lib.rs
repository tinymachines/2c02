//! The 2C02's netlist as a value: halfphi's blob, embedded at build
//! time, decoded on demand. The counts are measured by the build and
//! pinned by the tests against the handoff sketch's independent
//! measurement, so a parser change that silently altered the chip would
//! fail by number.

use std::sync::Arc;

pub use halfphi::{Engine, Netlist};

include!(concat!(env!("OUT_DIR"), "/counts.rs"));
include!(concat!(env!("OUT_DIR"), "/areas.rs"));

static BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/netlist.bin"));

/// Whether the die data was present at build time. Tests SKIP by name
/// when it was not; nothing else should ever branch on this.
pub fn available() -> bool {
    !BLOB.is_empty()
}

/// The decoded netlist. Panics by name when built data-free.
pub fn netlist() -> Arc<Netlist> {
    assert!(
        available(),
        "built without extern/visual2c02: run tools/fetch-netlist.sh and rebuild"
    );
    let mut nl = Netlist::decode(BLOB).expect("embedded blob decodes");
    // The reference's own rail-conflict special case, applied through
    // halfphi's generic hold: areas first (the vote's weights, computed
    // by build.rs the way wires.js computes them), then the list
    // extracted from the pinned chipsim.js.
    nl.set_node_areas(NODE_AREAS.into());
    nl.set_rail_conflict_holds(RAIL_CONFLICT_HOLDS);
    Arc::new(nl)
}
