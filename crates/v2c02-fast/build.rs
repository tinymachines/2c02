//! The recorder: one frame of the switch-level chip in the standard
//! world, reduced to one event word per dot. The table is what the
//! stepper's sequencer IS; nothing in it is typed. Written to OUT_DIR
//! (NC-SA-derived, never committed). Without the extern the table is
//! empty and every P3 test SKIPs by name.

use std::path::PathBuf;

use nes_bus::{DOTS_PER_LINE, LINES};
use v2c02_dots::{standard_world, Taps};

include!("src/events.rs");

const NODE_EVENTS: &[(&str, u16)] = &[
    ("load_vramaddr_v_hscroll_next", INC_X),
    ("load_vramaddr_v_vscroll_next", INC_Y),
    ("copy_vramaddr_hscroll", COPY_X),
    ("copy_vramaddr_vscroll", COPY_Y),
    ("set_vbl_flag", SET_VBL),
    ("vbl_clear_flags", CLR_FLAGS),
];

fn classify(addr: u16, in_sprite_window: bool) -> u16 {
    let kind = if addr < 0x2000 {
        if addr & 8 != 0 {
            2
        } else {
            1
        }
    } else if addr & 0x03c0 == 0x03c0 {
        3
    } else {
        0
    };
    match (in_sprite_window, kind) {
        (false, 0) => FETCH_NT,
        (false, 3) => FETCH_AT,
        (false, 1) => FETCH_PT_LO,
        (false, 2) => FETCH_PT_HI,
        (true, 1) => SPR_PT_LO,
        (true, 2) => SPR_PT_HI,
        (true, _) => SPR_GARBAGE,
        _ => unreachable!(),
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/events.rs");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("table.bin");
    if !v2c02_netlist::available() {
        std::fs::write(&out, []).unwrap();
        println!("cargo:warning=v2c02-fast: extern/visual2c02 not fetched; the table is empty and the P3 tests SKIP");
        return;
    }

    let mut h = standard_world();
    let taps = Taps::new(&h);
    let nl = h.ppu.engine.netlist().clone();
    let n = |s: &str| nl.node(s).unwrap_or_else(|| panic!("node {s}"));
    let nodes: Vec<_> = NODE_EVENTS.iter().map(|(name, bit)| (n(name), *bit)).collect();
    let clear_spr_ptr = n("clear_spr_ptr");

    // Align to the frame boundary the way the dot capture does: the
    // last dot of the pre-render line, which only an even frame has.
    while !(taps.bus(&h, &taps.vpos) == 261 && taps.bus(&h, &taps.hpos) == 340) {
        h.half_step();
    }

    // The alignment stops part way through that dot; consume the rest
    // of it so recording starts on the first half-step of (0, 0).
    while taps.bus(&h, &taps.vpos) == 261 && taps.bus(&h, &taps.hpos) == 340 {
        h.half_step();
    }

    let total = LINES * DOTS_PER_LINE;
    let mut table = vec![0u16; total];
    let mut key: Option<(usize, usize)> = None;
    let mut cur = 0u16;
    let mut spr_start: Option<usize> = None;
    let mut seen = 0usize;
    // Sample the current half-step, then step: every half-step of every
    // dot is seen exactly once, the last dot flushed when the frame
    // rolls over.
    loop {
        let hp = taps.bus(&h, &taps.hpos) as usize;
        let vp = taps.bus(&h, &taps.vpos) as usize;
        if key != Some((vp, hp)) {
            if let Some((pv, ph)) = key {
                table[pv * DOTS_PER_LINE + ph] = cur;
                seen += 1;
                if (pv, ph) == (LINES - 1, DOTS_PER_LINE - 1) {
                    break;
                }
                // With rendering on the frames alternate parity, and the
                // one after an even-frame alignment is odd: it ends at
                // dot 339 of the pre-render line. Discard it and record
                // the even frame that follows, which has every dot.
                if (pv, ph) == (LINES - 1, DOTS_PER_LINE - 2) && (vp, hp) == (0, 0) {
                    table.iter_mut().for_each(|e| *e = 0);
                    seen = 0;
                }
            }
            if hp == 0 {
                spr_start = None;
            }
            key = Some((vp, hp));
            cur = 0;
        }
        let p = h.pins();
        if h.ppu.engine.is_high(clear_spr_ptr) && hp > 200 && spr_start.is_none() {
            spr_start = Some(hp);
        }
        let in_spr = spr_start.is_some_and(|s| hp >= s && hp < s + 64);
        if p.ale {
            cur |= classify(((p.a_hi as u16) << 8) | p.ad as u16, in_spr);
        }
        if !p.rd_n {
            cur |= RD;
        }
        for &(id, bit) in &nodes {
            if h.ppu.engine.is_high(id) {
                cur |= bit;
            }
        }
        h.half_step();
    }
    assert_eq!(seen, total, "the recorded frame is not {LINES} x {DOTS_PER_LINE} dots");

    let bytes: Vec<u8> = table.iter().flat_map(|e| e.to_le_bytes()).collect();
    std::fs::write(&out, bytes).unwrap();
    let fetches = table.iter().filter(|e| **e & (FETCH_NT | FETCH_AT | FETCH_PT_LO | FETCH_PT_HI) != 0).count();
    let incx = table.iter().filter(|e| **e & INC_X != 0).count();
    println!(
        "cargo:warning=v2c02-fast: table recorded, {total} dots, {fetches} background fetches, {incx} INC_X, at half-step {}",
        h.half_steps
    );

    // The second measurement: the palette RAM as the chip HOLDS it, read
    // back through $2007 in vblank with an access-width of idle between
    // accesses (the pacing P2 found the register file needs), twice, and
    // the two passes must agree. Not the constant the world wrote: the
    // chip applies the $2006 low write to v with a delay
    // (`delayed_write_2006_low`), and the world's first back-to-back
    // $2007 write went to the stale address, so what the RAM holds is a
    // measurement, and the picture is rendered from it.
    while taps.bus(&h, &taps.vpos) != 242 {
        h.half_step();
    }
    let mut passes: Vec<[u8; 32]> = Vec::new();
    for _ in 0..2 {
        h.write(6, 0x3f);
        h.wait(24);
        h.write(6, 0x00);
        h.wait(24);
        let mut pal = [0u8; 32];
        for p in pal.iter_mut() {
            *p = h.read(7);
            h.wait(24);
        }
        passes.push(pal);
    }
    assert_eq!(passes[0], passes[1], "the paced palette read-back is not stable across two passes");
    let pal_out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("palette.bin");
    std::fs::write(&pal_out, passes[0]).unwrap();
    let stray: Vec<String> = h.vram_writes.iter().map(|(a, v)| format!("${a:04x}<-{v:02x}")).collect();
    println!(
        "cargo:warning=v2c02-fast: palette RAM as held: {}; writes the world sent to VRAM instead of the palette: {}",
        passes[0].iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" "),
        if stray.is_empty() { "none".to_string() } else { stray.join(" ") }
    );
}
