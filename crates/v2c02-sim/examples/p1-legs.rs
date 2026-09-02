//! Calibrate the DAC leg semantics empirically: run one full scanline
//! and record which legs are active at each hpos, then read the map off
//! the known line geography (sync, burst, blank, picture).
use v2c02_sim::{harness::Harness, Ppu};

fn vram(a: u16) -> u8 { (((a >> 4) ^ a) & 0xff) as u8 }
const PALETTE: [u8; 16] = [0x0f, 0x16, 0x2a, 0x12, 0x0f, 0x28, 0x14, 0x02,
                           0x0f, 0x26, 0x1a, 0x31, 0x0f, 0x30, 0x27, 0x06];

fn main() {
    let ppu = Ppu::power_on();
    let nl = ppu.engine.netlist().clone();
    let n = |s: &str| nl.node(s).unwrap();
    let hpos: Vec<_> = (0..9).map(|i| n(&format!("hpos{i}"))).collect();
    let vpos: Vec<_> = (0..9).map(|i| n(&format!("vpos{i}"))).collect();
    let pal_d: Vec<_> = (0..6).map(|i| n(&format!("pal_d{i}_out"))).collect();
    let legs = ["vid_sync_h","vid_sync_l","vid_burst_h","vid_burst_l",
                "vid_luma0_h","vid_luma0_l","vid_luma1_h","vid_luma1_l",
                "vid_luma2_h","vid_luma2_l","vid_luma3_h","vid_luma3_l","vid_emph"];
    let leg_ids: Vec<_> = legs.iter().map(|s| n(s)).collect();

    let mut h = Harness::new(ppu, vram);
    h.wait(712_100);
    h.read(2);
    for (reg, val) in [(0u8,0x00u8),(1,0x00),(6,0x3f),(6,0x00)] { h.write(reg, val); }
    for v in PALETTE { h.write(7, v); }
    for (reg, val) in [(6u8,0x20u8),(6,0x00),(5,0x00),(5,0x00),(1,0x0a)] { h.write(reg, val); }
    let bus = |h: &Harness, ids: &[halfphi::NodeId]| -> u32 {
        ids.iter().enumerate().map(|(i, &id)| (h.ppu.engine.is_high(id) as u32) << i).sum()
    };
    while !(bus(&h, &vpos) == 40 && bus(&h, &hpos) == 0) { h.half_step(); }
    // One full scanline: per hpos, the set of legs seen and the pal values.
    let mut seen: Vec<(u32, u16, Vec<u32>)> = Vec::new(); // hpos -> legmask set, pal set
    for _ in 0..341 * 8 {
        h.half_step();
        let hp = bus(&h, &hpos);
        let mask: u16 = leg_ids.iter().enumerate()
            .map(|(i, &id)| (h.ppu.engine.is_high(id) as u16) << i).sum();
        let pal = bus(&h, &pal_d);
        match seen.last_mut() {
            Some((p, m, pals)) if *p == hp => { *m |= mask; if !pals.contains(&pal) { pals.push(pal); } }
            _ => seen.push((hp, mask, vec![pal])),
        }
    }
    // Compress runs with identical leg masks.
    let mut i = 0;
    while i < seen.len() {
        let j = (i..seen.len()).take_while(|&k| seen[k].1 == seen[i].1).last().unwrap();
        let names: Vec<&str> = (0..13).filter(|b| seen[i].1 >> b & 1 != 0).map(|b| legs[b]).collect();
        println!("hpos {:3}..{:3}: legs {:?}", seen[i].0, seen[j].0, names);
        i = j + 1;
    }
}
