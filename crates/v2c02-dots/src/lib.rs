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
    for (reg, val) in [(0u8, 0x00u8), (1, 0x00), (6, 0x3f), (6, 0x00)] {
        h.write(reg, val);
    }
    for v in PALETTE {
        h.write(7, v);
    }
    for (reg, val) in [(6u8, 0x20u8), (6, 0x00), (5, 0x00), (5, 0x00), (1, 0x0a)] {
        h.write(reg, val);
    }
    h
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
    h.write(1, 0x00);
    h.wait(24);
    // The $2006 pair is waited out (the delayed low write), then each
    // triple is written back to back. Measured (docs/p3-report.md): a
    // $2007 palette write with an access-width of idle after it lands as
    // data OR the low byte of v, while back-to-back writes land as
    // written; the read-back beside the golden records what held.
    for (p, cols) in SPRITE_PALETTE.iter().enumerate() {
        h.write(6, 0x3f);
        h.wait(24);
        h.write(6, 0x11 + 4 * p as u8);
        h.wait(48);
        for &c in cols {
            h.write(7, c);
        }
        h.wait(24);
    }
    h.write(3, 0x00);
    h.wait(24);
    for &b in sprite_oam().iter() {
        h.write(4, b);
        h.wait(24);
    }
    for (reg, val) in [(6u8, 0x20u8), (6, 0x00), (5, 0x00), (5, 0x00)] {
        h.write(reg, val);
        h.wait(48);
    }
    h.write(1, 0x1e);
    h
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
