//! OAM corruption, measured: fill OAM[0..64] with the identity pattern
//! (paced writes), enable rendering, write OAMADDR mid-visible-frame,
//! blank again, read OAM back and print every byte that changed.
//! Folklore says the 8-byte row at (value & $F8) lands on row 0; this
//! prints what the silicon actually does.

use v2c02_sim::harness::Harness;
use v2c02_sim::Ppu;

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

fn main() {
    let mut h = Harness::new(Ppu::power_on(), vram);
    h.wait(712_100);
    for (rw, reg, val) in [(true, 2, 0x00), (false, 0, 0x00), (false, 1, 0x00)] {
        h.cpu_access(rw, reg, val);
    }
    // Identity fill, paced: OAM[i] = i. Sprite y bytes 0..63 keep the
    // garbage sprites off the visible lines' early rows only barely,
    // but rendering correctness is not this probe's subject.
    h.cpu_access(false, 3, 0x00);
    for i in 0..64u8 {
        h.cpu_access(false, 4, i);
        h.wait(24);
    }
    let read_oam = |h: &mut Harness, upto: u8| -> Vec<u8> {
        (0..upto)
            .map(|i| {
                h.cpu_access(false, 3, i);
                h.wait(24);
                let v = h.read(4);
                h.wait(24);
                v
            })
            .collect()
    };
    let before = read_oam(&mut h, 64);
    println!("before: {:02x?}", &before[..16]);

    // The sharper trigger: OAMADDR nonzero when rendering STARTS.
    // Set it during blanking, then enable rendering with it standing.
    h.cpu_access(false, 3, 0x28);
    h.cpu_access(false, 1, 0x1e);
    let nl = h.ppu.engine.netlist().clone();
    let n = |name: &str| nl.node(name).unwrap_or_else(|| panic!("node {name}"));
    let vpos: Vec<_> = (0..9).map(|i| n(&format!("vpos{i}"))).collect();
    let bits = |h: &Harness, ns: &[halfphi::NodeId]| -> u32 {
        ns.iter()
            .enumerate()
            .map(|(i, &nd)| (h.ppu.engine.is_high(nd) as u32) << i)
            .sum()
    };
    // Let a whole frame render with OAMADDR standing at $28, then
    // blank and read back.
    while bits(&h, &vpos) != 245 {
        h.half_step();
    }
    while bits(&h, &vpos) == 245 {
        h.half_step();
    }
    while bits(&h, &vpos) != 245 {
        h.half_step();
    }
    h.cpu_access(false, 1, 0x00);
    let after = read_oam(&mut h, 64);
    println!("after:  {:02x?}", &after[..16]);
    let mut diffs = Vec::new();
    for i in 0..64 {
        if before[i] != after[i] {
            diffs.push(format!("[{i}] {:02x}->{:02x}", before[i], after[i]));
        }
    }
    println!("changed bytes ({}): {}", diffs.len(), diffs.join(" "));
}
