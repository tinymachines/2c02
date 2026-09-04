//! The rung 0 half of the palette write-path instrument: the exact
//! sequence gen-pal-golden.js ran on the reference (the P1 warm-up, a
//! $2002 read, ctrl and mask zero, a $2006 pair to $3F11 waited out,
//! then ONE $2007 write of $21 and 96 idle half-steps), every node
//! compared against the golden on every half-step. Prints the first
//! half-step that differs and the named nodes that differ on it and on
//! the half-steps after, so the mechanism can be read off the names.
//! Measurement only; the fix, if any, is halfphi's.

use v2c02_sim::harness::Harness;
use v2c02_sim::Ppu;

fn vram(a: u16) -> u8 {
    (((a >> 4) ^ a) & 0xff) as u8
}

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/golden-trace/golden-2c02-pal.txt");
    let text = std::fs::read_to_string(path).expect("tools/golden-trace/golden-2c02-pal.txt (run gen-pal-golden.js)");
    let mut lines = text.lines();
    let header = lines.next().unwrap();
    let fields: Vec<&str> = header.split_whitespace().collect();
    let start: u64 = fields[fields.iter().position(|f| *f == "start").unwrap() + 1].parse().unwrap();
    let golden: Vec<&str> = lines.collect();
    println!("{header}");

    // CHARGE_RULE=any replays under the rule halfphi used before 0.1.6
    // (one charged member makes an undriven group high), which is the
    // divergence; the default is the netlist's declared rule.
    let rule = match std::env::var("CHARGE_RULE").as_deref() {
        Ok("any") => halfphi::ChargeRule::AnyHigh,
        _ => halfphi::ChargeRule::AreaVote,
    };
    println!("charge rule: {rule:?}");
    let mut h = Harness::new(Ppu::power_on_with(v2c02_netlist::netlist_with_charge_rule(rule)), vram);
    h.wait(712_100);
    h.read(2);
    h.write(0, 0);
    h.write(1, 0);
    h.write(6, 0x3f);
    h.wait(48);
    h.write(6, 0x11);
    h.wait(96);
    assert_eq!(h.half_steps, start, "the two sequences are not at the same half-step before the write");

    let nl = h.ppu.engine.netlist().clone();
    let name = |i: usize| nl.name_of(i as halfphi::NodeId).unwrap_or("(unnamed)").to_string();
    let mut states: Vec<String> = Vec::new();
    for counter in (1..=24u32).rev() {
        h.access_edge(false, 7, 0x21, counter);
        h.half_step();
        states.push(h.ppu.state_line());
    }
    h.end_access();
    for _ in 0..96 {
        h.half_step();
        states.push(h.ppu.state_line());
    }
    assert_eq!(states.len(), golden.len(), "state count");

    let mut first: Option<usize> = None;
    for (k, (got, want)) in states.iter().zip(&golden).enumerate() {
        let diff: Vec<usize> = got.bytes().zip(want.bytes()).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i).collect();
        if diff.is_empty() {
            continue;
        }
        if first.is_none() {
            first = Some(k);
            println!(
                "first divergence at window state {k} (half-step {}, access edge {})",
                start + k as u64 + 1,
                if k < 24 { format!("{}", 24 - k) } else { format!("idle +{}", k - 23) }
            );
        }
        if k < first.unwrap() + 12 || k % 12 == 0 {
            let named: Vec<String> = diff.iter().take(24).map(|&i| {
                let ours = states[k].as_bytes()[i] as char;
                format!("{}={ours}", name(i))
            }).collect();
            println!("  state {k}: {} nodes differ: {}{}", diff.len(), named.join(" "), if diff.len() > 24 { " ..." } else { "" });
        }
    }
    match first {
        None => println!("no divergence in the window: rung 0 and the reference agree node for node through this write"),
        Some(_) => {
            // The palette write's outcome on rung 0, for the record.
            h.write(6, 0x3f);
            h.wait(48);
            h.write(6, 0x11);
            h.wait(96);
            let a = h.read(7);
            println!("rung 0 read back $3F11 = {a:02x} (wrote 21)");
        }
    }
}
