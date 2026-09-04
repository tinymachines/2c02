//! From switches to dots: the standard P1 world (the register program
//! from the golden generator), frame capture off the palette output
//! bus, and the sample-exact DAC comparison.
//!
//! Everything here leans on two measurements made before this file was
//! written (docs/p1-report.md):
//!
//! - `pal_d0..5_out` carries the pixel's colour during the pclk1 phase
//!   of each dot and precharges to zero during pclk0.
//! - The video DAC has eleven level legs whose meanings were calibrated
//!   from a scanline's known geography, NOT from their names:
//!   `vid_sync_l` is the sync tip, **`vid_sync_h` is the blanking
//!   level** (which is why the $xE/$xF blacks assert it: the
//!   transcribed table says those columns output the blank voltage),
//!   `vid_burst_l/h` are the burst's two levels, and `vid_lumaR_l/h`
//!   are the table's LOW[R]/HIGH[R] columns. One half-step is one grid
//!   sample: the master half-clock IS 12 x f_sc.

use halfphi::NodeId;
use nes_bus::{DotFrame, FrameParity, ACTIVE_ROWS, DOTS_PER_LINE};
use ntsc_source_nes::levels;
use v2c02_sim::harness::Harness;
use v2c02_sim::Ppu;

/// The shared P1 world: the VRAM function and register program the
/// golden generator scripts in JS, one definition per language, both
/// quoting this file's constants in their comments.
pub fn vram(a: u16) -> u8 {
    (((a >> 4) ^ a) & 0xff) as u8
}

pub const PALETTE: [u8; 16] = [
    0x0f, 0x16, 0x2a, 0x12, 0x0f, 0x28, 0x14, 0x02,
    0x0f, 0x26, 0x1a, 0x31, 0x0f, 0x30, 0x27, 0x06,
];

/// Warm up (the measured 712,010-half-step reset gate plus margin) and
/// run the register program: toggle reset via a $2002 read, ctrl and
/// mask zero, the palette loaded, the address parked in the nametable,
/// scroll zeroed, background rendering on.
pub fn standard_world() -> Harness {
    let mut h = Harness::new(Ppu::power_on(), vram);
    h.wait(712_100);
    h.read(2);
    for (reg, val, idle) in standard_program() {
        h.write(reg, val);
        h.wait(idle);
    }
    h
}

/// A register program: (register, value, half-steps of idle after the
/// access). The idle is the harness's pacing; the stepper's register
/// file ignores it.
pub type Program = Vec<(u8, u8, u64)>;

/// The standard world's register program after its $2002 read, in
/// order: the one sequence `standard_world` runs on the chip, the P1
/// golden generator scripts, and the stepper's register file runs on
/// itself. Each $2007 palette write is followed by an access width of
/// idle so the palette lands as written: back to back, both engines
/// lose the first value and land the rest one entry early (the write
/// probe, docs/p3-report.md), which is how the world was first
/// recorded; re-recorded paced on 2026-09-04.
pub fn standard_program() -> Program {
    let mut p = vec![(0u8, 0x00u8, 0u64), (1, 0x00, 0), (6, 0x3f, 0), (6, 0x00, 0)];
    p.extend(PALETTE.iter().map(|&v| (7, v, 24)));
    p.extend([(6, 0x20, 0), (6, 0x00, 0), (5, 0x00, 0), (5, 0x00, 0), (1, 0x0a, 0)]);
    p
}

/// The three colours of each sprite palette the sprite world writes to
/// $3F11.., $3F15.., $3F19.., $3F1D.. (entry 0 of each mirrors the
/// backdrop and is left alone). None appears in the background palette
/// as the chip holds it, so a sprite pixel is unmistakable in a dot diff.
pub const SPRITE_PALETTE: [[u8; 3]; 4] = [[0x21, 0x11, 0x01], [0x24, 0x15, 0x04], [0x29, 0x19, 0x09], [0x2c, 0x1c, 0x0c]];

/// The 64 sprites the sprite world loads, (y, tile, attribute, x) each;
/// the unused ones parked at y = $F0 as P2's scenario did. What each
/// exercises is in the comments; the tiles' patterns are whatever the
/// world's VRAM function yields for them.
pub fn sprite_oam() -> [u8; 256] {
    let mut oam = [0u8; 256];
    for s in oam.chunks_exact_mut(4) {
        s.copy_from_slice(&[0xf0, 0, 0, 0]);
    }
    let mut put = |i: usize, y: u8, tile: u8, attr: u8, x: u8| oam[i * 4..i * 4 + 4].copy_from_slice(&[y, tile, attr, x]);
    put(0, 90, 1, 0x00, 180); // P2's sprite 0, over solid background: the hit
    put(1, 40, 3, 0x01, 20); // palette 1
    put(2, 40, 3, 0x41, 28); // flipped horizontally
    put(3, 40, 3, 0x81, 36); // flipped vertically
    put(4, 40, 3, 0xc1, 44); // both flips
    put(5, 40, 5, 0x22, 52); // behind the background, palette 2
    for k in 0..9u8 {
        // nine on one line: the eighth is the last one drawn
        put(6 + k as usize, 120, 2 + k, k & 3, 16 + 24 * k);
    }
    put(15, 200, 4, 0x03, 250); // off the right edge
    put(16, 236, 6, 0x00, 100); // off the bottom
    put(17, 60, 2, 0x00, 100); // two overlapping: the lower index wins
    put(18, 62, 4, 0x01, 104);
    oam
}

/// The sprite world: the standard world with rendering paused, the four
/// sprite palettes and the 64 sprites loaded with an access-width of
/// idle after every write (P2's pacing finding) and the delayed $2006
/// load waited out (P3's), t restored the way the standard world sets
/// it, and rendering resumed with sprites on and no left-edge clipping
/// (mask $1E).
pub fn sprite_world() -> Harness {
    let mut h = standard_world();
    for (reg, val, idle) in sprite_program() {
        h.write(reg, val);
        h.wait(idle);
    }
    h
}

/// The sprite world's register program after the standard world's, as
/// (register, value, half-steps of idle after the access): the idle is
/// the harness's pacing (P2's finding for OAM writes; a $2006 pair
/// waited out; the palette written with an access width of idle after
/// each entry, which lands every entry as written now that halfphi
/// 0.1.6 resolves the chip's undriven groups the reference's way,
/// docs/p3-report.md). The register file ignores the idle; the read-back
/// beside the golden records what the chip held.
pub fn sprite_program() -> Vec<(u8, u8, u64)> {
    let mut p = vec![(1u8, 0x00u8, 24u64)];
    for (i, cols) in SPRITE_PALETTE.iter().enumerate() {
        p.push((6, 0x3f, 24));
        p.push((6, 0x11 + 4 * i as u8, 48));
        for &c in cols {
            p.push((7, c, 24));
        }
    }
    p.push((3, 0x00, 24));
    p.extend(sprite_oam().iter().map(|&b| (4, b, 24)));
    p.extend([(6, 0x20, 48), (6, 0x00, 48), (5, 0x00, 48), (5, 0x00, 48), (1, 0x1e, 0)]);
    p
}

/// A register write scheduled inside a captured frame: the access
/// starts on the first half-step of dot (vpos, hpos).
#[derive(Clone, Copy, Debug)]
pub struct TimedWrite {
    pub vpos: u16,
    pub hpos: u16,
    pub reg: u8,
    pub val: u8,
}

/// The register program of the scroll world, before the frame: the
/// standard world's, then the background table at $1000 and the
/// nametable at $2400 ($2000 = $11), a scroll of x = $25 (coarse 4,
/// fine 5) and y = $13 (coarse 2, fine 3), background on with the
/// left column shown ($2001 = $0A).
pub const SCROLL_PROGRAM: [(u8, u8, u64); 4] = [(0, 0x11, 48), (5, 0x25, 48), (5, 0x13, 48), (1, 0x0a, 48)];

/// The mid-frame writes of the scroll world, the classic two: a
/// horizontal split at line 100 (one $2005 write changes fine x at once
/// and coarse x at the next horizontal copy), and a full scroll change
/// at line 160 by the $2006/$2005/$2005/$2006 sequence, each access
/// four dots after the last.
pub const SCROLL_WRITES: [TimedWrite; 5] = [
    TimedWrite { vpos: 100, hpos: 10, reg: 5, val: 0x80 },
    TimedWrite { vpos: 160, hpos: 10, reg: 6, val: 0x08 },
    TimedWrite { vpos: 160, hpos: 14, reg: 5, val: 0x40 },
    TimedWrite { vpos: 160, hpos: 18, reg: 5, val: 0x10 },
    TimedWrite { vpos: 160, hpos: 22, reg: 6, val: 0xa2 },
];

/// The scroll world: the standard world with rendering paused, its
/// register program applied with the $2006 pair's idle, and rendering
/// resumed scrolled. The mid-frame writes are the capture's business.
pub fn scroll_world() -> Harness {
    let mut h = standard_world();
    h.write(1, 0x00);
    h.wait(48);
    for (reg, val, idle) in SCROLL_PROGRAM {
        h.write(reg, val);
        h.wait(idle);
    }
    h
}

/// `capture` with register writes performed inside the frame at their
/// scheduled dots, the 24-edge access interleaved with the sampling so
/// no half-step goes unseen. Returns the capture and, for each write,
/// the half-step at which its access started.
pub fn capture_with_writes(h: &mut Harness, rows: usize, writes: &[TimedWrite]) -> (Captured, Vec<u64>) {
    let taps = Taps::new(h);
    while !(taps.bus(h, &taps.vpos) == 261 && taps.bus(h, &taps.hpos) == 340) {
        h.half_step();
    }
    let nl = h.ppu.engine.netlist().clone();
    let hit_node = nl.node("spr0_hit").expect("node spr0_hit");
    let ovf_node = nl.node("spr_overflow").expect("node spr_overflow");
    let mut dots = DotFrame::filled(FrameParity::Even, 0x0f, 0);
    let mut trace = Vec::new();
    let mut seen_pclk1 = false;
    let mut spr0_hit = None;
    let mut spr_overflow = None;
    let (mut was_hit, mut was_ovf) = (h.ppu.engine.is_high(hit_node), h.ppu.engine.is_high(ovf_node));
    let mut next = 0usize;
    let mut started = Vec::new();
    // An access in flight: (reg, val, edges left).
    let mut access: Option<(u8, u8, u32)> = None;
    loop {
        if let Some((reg, val, counter)) = access {
            h.access_edge(false, reg, val, counter);
            access = if counter > 1 { Some((reg, val, counter - 1)) } else { None };
            if access.is_none() {
                h.end_access();
            }
        }
        h.half_step();
        let hp = taps.bus(h, &taps.hpos) as u16;
        let vp = taps.bus(h, &taps.vpos) as u16;
        if vp as usize >= rows && vp != 261 {
            break;
        }
        if access.is_none() && next < writes.len() && (vp, hp) == (writes[next].vpos, writes[next].hpos) {
            access = Some((writes[next].reg, writes[next].val, 24));
            started.push(h.half_steps);
            next += 1;
        }
        let (hit, ovf) = (h.ppu.engine.is_high(hit_node), h.ppu.engine.is_high(ovf_node));
        if hit && !was_hit && spr0_hit.is_none() {
            spr0_hit = Some((vp, hp));
        }
        if ovf && !was_ovf && spr_overflow.is_none() {
            spr_overflow = Some((vp, hp));
        }
        was_hit = hit;
        was_ovf = ovf;
        trace.push((hp, vp, taps.leg_mask(h), h.ppu.engine.is_high(taps.emph)));
        let p1 = h.ppu.engine.is_high(taps.pclk1);
        if p1 && seen_pclk1 && (vp as usize) < ACTIVE_ROWS && (hp as usize) < DOTS_PER_LINE - 1 {
            let colour = taps.bus(h, &taps.pal_d) as u8;
            dots.set(vp as usize, hp as usize + 1, colour, 0);
            seen_pclk1 = false;
        } else {
            seen_pclk1 = p1;
        }
    }
    assert_eq!(next, writes.len(), "not every scheduled write started");
    (Captured { dots, trace, spr0_hit, spr_overflow }, started)
}

/// Read the palette RAM (32) and OAM (256) back out of the chip in
/// vblank, paced, the way the P3 recorder does, so a world's contents
/// are a measurement. Leaves the chip in vblank with rendering as it was.
pub fn read_back_palette_and_oam(h: &mut Harness) -> ([u8; 32], [u8; 256]) {
    let taps = Taps::new(h);
    while taps.bus(h, &taps.vpos) != 242 {
        h.half_step();
    }
    h.write(6, 0x3f);
    h.wait(24);
    h.write(6, 0x00);
    h.wait(48);
    let mut pal = [0u8; 32];
    for p in pal.iter_mut() {
        *p = h.read(7);
        h.wait(24);
    }
    let mut oam = [0u8; 256];
    for (i, o) in oam.iter_mut().enumerate() {
        h.write(3, i as u8);
        h.wait(24);
        *o = h.read(4);
        h.wait(24);
    }
    h.write(3, 0);
    h.wait(24);
    // Reading $2007 moved v; put t back the way the world had it so the
    // pre-render copies restore the picture.
    for (reg, val) in [(6u8, 0x20u8), (6, 0x00), (5, 0x00), (5, 0x00)] {
        h.write(reg, val);
        h.wait(48);
    }
    (pal, oam)
}

/// The taps this crate reads.
pub struct Taps {
    pub hpos: [NodeId; 9],
    pub vpos: [NodeId; 9],
    pub pal_d: [NodeId; 6],
    pub pclk1: NodeId,
    /// The eleven level legs, index = `Leg` discriminant.
    pub legs: [NodeId; 11],
    pub emph: NodeId,
}

/// The DAC's level legs, in this crate's canonical order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Leg {
    SyncTip = 0, // vid_sync_l
    Blank,       // vid_sync_h: the name lies, the geography does not
    BurstLow,
    BurstHigh,
    Luma0Low,
    Luma0High,
    Luma1Low,
    Luma1High,
    Luma2Low,
    Luma2High,
    Luma3Low,
    Luma3High,
}

const LEG_NAMES: [&str; 11] = [
    "vid_sync_l",
    "vid_sync_h",
    "vid_burst_l",
    "vid_burst_h",
    "vid_luma0_l",
    "vid_luma0_h",
    "vid_luma1_l",
    "vid_luma1_h",
    "vid_luma2_l",
    "vid_luma2_h",
    "vid_luma3_l",
];

/// The voltage each leg selects, straight from the transcribed table's
/// generated constants (the eleventh, luma3_h, is read separately
/// because arrays and enums disagree about the count by one).
pub fn leg_voltage(leg: usize) -> f32 {
    [
        levels::SYNC,
        levels::BLANK,
        levels::BURST_LOW,
        levels::BURST_HIGH,
        levels::LOW[0],
        levels::HIGH[0],
        levels::LOW[1],
        levels::HIGH[1],
        levels::LOW[2],
        levels::HIGH[2],
        levels::LOW[3],
        levels::HIGH[3],
    ][leg]
}

impl Taps {
    pub fn new(h: &Harness) -> Taps {
        let nl = h.ppu.engine.netlist().clone();
        let n = |s: &str| nl.node(s).unwrap_or_else(|| panic!("node {s}"));
        let mut legs = [0 as NodeId; 11];
        for (i, name) in LEG_NAMES.iter().enumerate() {
            legs[i] = n(name);
        }
        Taps {
            hpos: std::array::from_fn(|i| n(&format!("hpos{i}"))),
            vpos: std::array::from_fn(|i| n(&format!("vpos{i}"))),
            pal_d: std::array::from_fn(|i| n(&format!("pal_d{i}_out"))),
            pclk1: n("pclk1"),
            legs,
            emph: n("vid_emph"),
        }
    }

    pub fn bus(&self, h: &Harness, ids: &[NodeId]) -> u32 {
        ids.iter()
            .enumerate()
            .map(|(i, &n)| (h.ppu.engine.is_high(n) as u32) << i)
            .sum()
    }

    /// The twelve level legs as a bitmask (luma3_h read by name).
    pub fn leg_mask(&self, h: &Harness) -> u16 {
        let mut m: u16 = self
            .legs
            .iter()
            .enumerate()
            .map(|(i, &n)| (h.ppu.engine.is_high(n) as u16) << i)
            .sum();
        let nl = h.ppu.engine.netlist();
        if h.ppu.engine.is_high(nl.node("vid_luma3_h").unwrap()) {
            m |= 1 << 11;
        }
        m
    }
}

/// One captured frame: the dots, and per half-step the leg mask over
/// the capture, for the DAC comparison.
pub struct Captured {
    pub dots: DotFrame,
    /// Per half-step from the frame's first: (hpos, vpos, leg mask,
    /// emph level).
    pub trace: Vec<(u16, u16, u16, bool)>,
    /// The (vpos, hpos) at which `spr0_hit` first rose in the captured
    /// frame, and the same for `spr_overflow`; `None` if never.
    pub spr0_hit: Option<(u16, u16)>,
    pub spr_overflow: Option<(u16, u16)>,
}

/// Run to the frame boundary and capture `rows` scanlines of dots (and
/// the full leg trace). The colour of dot h on a visible line is the
/// pal bus sampled in the dot's pclk1 phase, mapped to DotFrame's
/// convention (active starts at dot 1: dot = hpos + 1 for the visible
/// 0..256). Rows beyond the captured range keep the backdrop.
pub fn capture(h: &mut Harness, rows: usize) -> Captured {
    let taps = Taps::new(h);
    // Align to the frame boundary: the last half-step of vpos 261.
    while !(taps.bus(h, &taps.vpos) == 261 && taps.bus(h, &taps.hpos) == 340) {
        h.half_step();
    }
    let nl = h.ppu.engine.netlist().clone();
    let hit_node = nl.node("spr0_hit").expect("node spr0_hit");
    let ovf_node = nl.node("spr_overflow").expect("node spr_overflow");
    let mut dots = DotFrame::filled(FrameParity::Even, 0x0f, 0);
    let mut trace = Vec::new();
    let mut seen_pclk1 = false;
    let mut spr0_hit = None;
    let mut spr_overflow = None;
    let (mut was_hit, mut was_ovf) = (h.ppu.engine.is_high(hit_node), h.ppu.engine.is_high(ovf_node));
    loop {
        h.half_step();
        let hp = taps.bus(h, &taps.hpos) as u16;
        let vp = taps.bus(h, &taps.vpos) as u16;
        if vp as usize >= rows && vp != 261 {
            break;
        }
        let (hit, ovf) = (h.ppu.engine.is_high(hit_node), h.ppu.engine.is_high(ovf_node));
        if hit && !was_hit && spr0_hit.is_none() {
            spr0_hit = Some((vp, hp));
        }
        if ovf && !was_ovf && spr_overflow.is_none() {
            spr_overflow = Some((vp, hp));
        }
        was_hit = hit;
        was_ovf = ovf;
        trace.push((hp, vp, taps.leg_mask(h), h.ppu.engine.is_high(taps.emph)));
        // Sample the colour once per dot, on the second pclk1 half-step.
        let p1 = h.ppu.engine.is_high(taps.pclk1);
        if p1 && seen_pclk1 && (vp as usize) < ACTIVE_ROWS && (hp as usize) < DOTS_PER_LINE - 1 {
            let colour = taps.bus(h, &taps.pal_d) as u8;
            dots.set(vp as usize, hp as usize + 1, colour, 0);
            seen_pclk1 = false;
        } else {
            seen_pclk1 = p1;
        }
    }
    Captured { dots, trace, spr0_hit, spr_overflow }
}
