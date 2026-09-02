use v2c02_sim::Ppu;
fn main() {
    let golden = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/golden-trace/golden-2c02.txt"),
    ).unwrap();
    let mut ppu = Ppu::power_on();
    let nl = ppu.engine.netlist().clone();
    let mut ever: std::collections::BTreeMap<usize, usize> = Default::default(); // node -> last step
    for (step, want) in golden.lines().skip(1).enumerate() {
        if step > 0 { ppu.half_step(); }
        let got = ppu.state_line();
        for (i, (a, b)) in got.bytes().zip(want.bytes()).enumerate() {
            if a != b { *ever.entry(i).or_default() = step; }
        }
    }
    println!("nodes that ever differ: {}", ever.len());
    for (i, last) in &ever {
        println!("  node {i:5} last diff at step {last:3}  {:?}", nl.name_of(*i as u16));
    }
}
