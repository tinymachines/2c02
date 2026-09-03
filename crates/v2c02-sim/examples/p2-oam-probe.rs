//! Why do $2004 writes not land? The narrow probe: after the P1
//! warm-up and register program (rendering off), write one byte to
//! OAM[0] and watch oam_write_disable and the sprite address lines at
//! every edge of the access, then read the byte back; repeat at a
//! different frame phase. Measurement only; nothing here is a claim.

use v2c02_sim::harness::Harness;
use v2c02_sim::Ppu;

fn vram(_a: u16) -> u8 {
    0
}

fn main() {
    let mut h = Harness::new(Ppu::power_on(), vram);
    h.wait(712_100);
    // Minimal register state: toggle reset, ctrl/mask zero.
    for (rw, reg, val) in [(true, 2, 0x00), (false, 0, 0x00), (false, 1, 0x00)] {
        h.cpu_access(rw, reg, val);
    }

    let nl = h.ppu.engine.netlist().clone();
    let n = |name: &str| nl.node(name).unwrap_or_else(|| panic!("node {name}"));
    let owd = n("oam_write_disable");
    let hpos: Vec<_> = (0..9).map(|i| n(&format!("hpos{i}"))).collect();
    let vpos: Vec<_> = (0..9).map(|i| n(&format!("vpos{i}"))).collect();
    let bits = |h: &Harness, ns: &[halfphi::NodeId]| -> u32 {
        ns.iter()
            .enumerate()
            .map(|(i, &nd)| (h.ppu.engine.is_high(nd) as u32) << i)
            .sum()
    };

    // Park inside vblank first: run until in_vblank is high and vpos
    // reads 245, then probe there as phase 0.
    let in_vbl = n("in_vblank");
    while !(h.ppu.engine.is_high(in_vbl) && bits(&h, &vpos) == 245) {
        h.half_step();
    }
    for phase in 0..3 {
        println!(
            "phase {phase}: vpos {} hpos {} oam_write_disable {}",
            bits(&h, &vpos),
            bits(&h, &hpos),
            h.ppu.engine.is_high(owd) as u8
        );
        // Quietust's own pacing (cpucmd.js): each $2004 write is
        // followed by a 24-edge idle before the next access.
        h.cpu_access(false, 3, 0x00);
        let vals = [0xabu8, 0xcd, 0xef, 0x12];
        for v in vals {
            h.cpu_access(false, 4, v);
            h.wait(24);
        }
        let mut back = [0u8; 4];
        for i in 0..4u8 {
            h.cpu_access(false, 3, i);
            h.wait(24);
            back[i as usize] = h.read(4);
            h.wait(24);
        }
        println!("  wrote {vals:02x?} with 24-edge idles, read back {back:02x?}");
        h.wait(200_000);
    }
}
