//! The 2C02's dot clock phase after its power-on recipe: the master
//! half-steps (clk_in toggles) at which pclk0 and pclk1 change over the
//! first hundred, so a console starting both chips on one master clock
//! knows where its dots fall relative to the 2A03's clk0.
use v2c02_sim::Ppu;
fn main() {
    let mut ppu = Ppu::power_on();
    let nl = ppu.engine.netlist().clone();
    let (p0, p1) = (nl.node("pclk0").unwrap(), nl.node("pclk1").unwrap());
    let mut prev = (ppu.engine.is_high(p0), ppu.engine.is_high(p1));
    let mut edges = Vec::new();
    for m in 1..=100u32 {
        ppu.half_step();
        let now = (ppu.engine.is_high(p0), ppu.engine.is_high(p1));
        if now != prev {
            edges.push((m, now.0 as u8, now.1 as u8));
            prev = now;
        }
    }
    println!("2C02 pclk0/pclk1 edges (master half-step, pclk0, pclk1) after power_on: {edges:?}");
}
