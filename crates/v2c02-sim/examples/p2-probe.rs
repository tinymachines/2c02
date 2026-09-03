//! The measurement P2's scripts are written from: the P1 warm-up and
//! register program, extended with sprite 0 in OAM and sprites enabled
//! over a designed world (every background tile solid, sprite 0 solid
//! at an authored position), then a free run watching the chip's own
//! flags and position counters. Prints, in half-steps after init:
//!
//! - the frame phase after the last register write (vpos, hpos),
//! - every rise of in_vblank and set_vbl_flag for two frames,
//! - the first rise of set_spr0_hit, with vpos/hpos at that half-step.
//!
//! Run before authoring anything about where the events land.

use v2c02_sim::harness::Harness;
use v2c02_sim::Ppu;

/// The designed world: tile 1 is solid in both bitplanes, every other
/// tile transparent; every nametable entry names tile 1; attributes 0.
fn vram(a: u16) -> u8 {
    let a = a & 0x3fff;
    match a {
        0x0010..=0x001f => 0xff,
        0x0000..=0x1fff => 0x00,
        _ => {
            let nt = a & 0x0fff;
            if nt & 0x03ff < 0x03c0 { 0x01 } else { 0x00 }
        }
    }
}

const PALETTE: [u8; 16] = [
    0x0f, 0x16, 0x2a, 0x12, 0x0f, 0x28, 0x14, 0x02,
    0x0f, 0x26, 0x1a, 0x31, 0x0f, 0x30, 0x27, 0x06,
];

/// Sprite 0's authored position and shape.
const SPR_Y: u8 = 90;
const SPR_TILE: u8 = 1;
const SPR_ATTR: u8 = 0;
const SPR_X: u8 = 180;

fn main() {
    let mut h = Harness::new(Ppu::power_on(), vram);
    h.wait(712_100);

    // The P1 register program, then P2's extension: OAM sprite 0 and
    // sprites enabled with no clipping.
    let mut writes: Vec<(bool, u8, u8)> = vec![(true, 2, 0x00), (false, 0, 0x00), (false, 1, 0x00)];
    writes.push((false, 6, 0x3f));
    writes.push((false, 6, 0x00));
    for v in PALETTE {
        writes.push((false, 7, v));
    }
    writes.extend([
        (false, 6, 0x20),
        (false, 6, 0x00),
        (false, 5, 0x00),
        (false, 5, 0x00),
        (false, 3, 0x00),
        (false, 4, SPR_Y),
        (false, 4, SPR_TILE),
        (false, 4, SPR_ATTR),
        (false, 4, SPR_X),
    ]);
    for (rw, reg, val) in writes {
        h.cpu_access(rw, reg, val);
        // A $2004 write completes in the idle after the access releases;
        // a back-to-back follower cancels it (measured, p2-oam-probe).
        // Real bus masters always leave this gap; the harness must too.
        if reg == 4 {
            h.wait(24);
        }
    }
    // Did the $2004 writes land? Read OAM[0..4] back before rendering.
    h.cpu_access(false, 3, 0x00);
    let mut oam_back = [0u8; 4];
    for i in 0..4u8 {
        // reads do not increment OAMADDR on the 2C02; step it by hand
        h.cpu_access(false, 3, i);
        oam_back[i as usize] = h.read(4);
    }
    println!("OAM[0..4] read back: {oam_back:?} (wrote [{SPR_Y}, {SPR_TILE}, {SPR_ATTR}, {SPR_X}])");
    h.cpu_access(false, 3, 0x00);
    h.cpu_access(false, 1, 0x1e);

    let nl = h.ppu.engine.netlist().clone();
    let n = |name: &str| nl.node(name).unwrap_or_else(|| panic!("node {name}"));
    let hpos: Vec<_> = (0..9).map(|i| n(&format!("hpos{i}"))).collect();
    let vpos: Vec<_> = (0..9).map(|i| n(&format!("vpos{i}"))).collect();
    let in_vbl = n("in_vblank");
    let set_vbl = n("set_vbl_flag");
    let set_hit = n("set_spr0_hit");
    let spr0_active = n("spr0_active");
    let in_range = n("sprite_in_range");
    let copy_spr = n("copy_sprite_to_sec_oam");
    let bits = |h: &Harness, ns: &[halfphi::NodeId]| -> u32 {
        ns.iter()
            .enumerate()
            .map(|(i, &nd)| (h.ppu.engine.is_high(nd) as u32) << i)
            .sum()
    };

    println!(
        "after writes: half_steps {} vpos {} hpos {}",
        h.half_steps,
        bits(&h, &vpos),
        bits(&h, &hpos)
    );

    // The reference's own checkpoints (gen-p2-probe.js), for the
    // side-by-side: spr0_p through the fetch and display lines.
    let sp: Vec<_> = (0..8).map(|i| n(&format!("spr0_p{i}"))).collect();
    let s_hit = n("spr0_hit");
    let checkpoints: [(u32, u32); 8] = [
        (SPR_Y as u32, 250),
        (SPR_Y as u32, 300),
        (SPR_Y as u32, 330),
        (SPR_Y as u32 + 1, 1),
        (SPR_Y as u32 + 1, 60),
        (SPR_Y as u32 + 1, 120),
        (SPR_Y as u32 + 1, 178),
        (SPR_Y as u32 + 1, 200),
    ];
    for (tv, th) in checkpoints {
        while !(bits(&h, &vpos) == tv && bits(&h, &hpos) >= th) {
            h.half_step();
        }
        println!(
            "vpos {tv} hpos {}: spr0_p {} spr0_hit {}",
            bits(&h, &hpos),
            bits(&h, &sp),
            h.ppu.engine.is_high(s_hit) as u8
        );
    }
}
