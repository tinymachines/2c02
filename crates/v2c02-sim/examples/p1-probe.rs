//! Measure before assuming: after warm-up + the register program, dump
//! hpos/vpos, pclk phases, the pal_d bus and every DAC leg per
//! half-step, to learn the pixel anatomy from the chip itself.
use v2c02_sim::{harness::Harness, Ppu};

fn vram(a: u16) -> u8 {
    (((a >> 4) ^ a) & 0xff) as u8
}

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
    let (pclk0, pclk1, rendering) = (n("pclk0"), n("pclk1"), n("hpos_lt_256_and_rendering"));

    let mut h = Harness::new(ppu, vram);
    h.wait(712_100);
    let status = h.read(2); // resets the $2005/$2006 write toggle
    eprintln!("$2002 = {status:02x}");
    for (reg, val) in [(0u8,0x00u8),(1,0x00),(6,0x3f),(6,0x00)] { h.write(reg, val); }
    for v in PALETTE { h.write(7, v); }
    // Read the palette back before pointing away: does it hold what
    // was written?
    h.read(2);
    h.write(6, 0x3f);
    h.write(6, 0x00);
    let dummy = h.read(7);
    h.read(2);
    h.write(6, 0x3f);
    h.write(6, 0x00);
    let mut back = Vec::new();
    for _ in 0..17 { back.push(h.read(7)); }
    eprintln!("dummy first read: {dummy:02x}; 17 reads from $3F00: {back:02x?}");
    for (reg, val) in [(6u8,0x20u8),(6,0x00),(5,0x00),(5,0x00),(1,0x0a)] { h.write(reg, val); }
    eprintln!("writes done at half-step {}", h.half_steps);
    // run to a visible scanline mid-frame
    let bus = |h: &Harness, ids: &[halfphi::NodeId]| -> u32 {
        ids.iter().enumerate().map(|(i, &n)| (h.ppu.engine.is_high(n) as u32) << i).sum()
    };
    while !(bus(&h, &vpos) == 30 && bus(&h, &hpos) == 60) { h.half_step(); }
    eprintln!("at vpos 30 hpos 60, rendering={}", h.ppu.engine.is_high(rendering));
    for _ in 0..48 {
        h.half_step();
        let leg_str: String = leg_ids.iter().map(|&id| if h.ppu.engine.is_high(id) {'#'} else {'.'}).collect();
        println!("h={:3} p0={} p1={} pal={:02x} legs={}",
            bus(&h, &hpos), h.ppu.engine.is_high(pclk0) as u8, h.ppu.engine.is_high(pclk1) as u8,
            bus(&h, &pal_d), leg_str);
    }
    println!("legend: {}", legs.join(","));
    println!("vram_writes captured: {}", h.vram_writes.len());
}
