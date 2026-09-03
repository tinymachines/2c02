//! The spr_d bus's rail wiring: which transistors can join the OAM data
//! lines to vcc or vss, and what gates them. The reference's chipsim
//! special-cases exactly these nodes when a group holds both rails;
//! this probe shows the topology that makes that necessary.

fn main() {
    let nl = v2c02_netlist::netlist();
    let mut sprd = Vec::new();
    for i in 0..8 {
        let name = format!("spr_d{i}");
        let id = nl.node(&name).unwrap_or_else(|| panic!("{name}"));
        sprd.push((name, id));
    }
    let (vcc, vss) = (nl.vcc(), nl.vss());
    for (name, id) in &sprd {
        let mut rail_paths = 0;
        let mut total = 0;
        for t in 0..nl.transistor_count() as halfphi::TransId {
            let (c1, c2) = (nl.transistor_c1(t), nl.transistor_c2(t));
            if c1 == *id || c2 == *id {
                total += 1;
                let other = if c1 == *id { c2 } else { c1 };
                if other == vcc || other == vss {
                    rail_paths += 1;
                    let g = nl.transistor_gate(t);
                    println!(
                        "{name}: t{t} to {} gated by {} ({})",
                        if other == vcc { "vcc" } else { "vss" },
                        g,
                        nl.name_of(g).unwrap_or("(unnamed)")
                    );
                }
            }
        }
        println!("{name}: {total} channel connections, {rail_paths} directly to a rail");
    }
}
