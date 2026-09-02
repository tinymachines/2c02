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
use ntsc_grid::FrameParity;
use ntsc_source_nes::{levels, DotFrame, ACTIVE_ROWS, DOTS_PER_LINE};
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
    let mut dots = DotFrame::filled(FrameParity::Even, 0x0f, 0);
    let mut trace = Vec::new();
    let mut seen_pclk1 = false;
    loop {
        h.half_step();
        let hp = taps.bus(h, &taps.hpos) as u16;
        let vp = taps.bus(h, &taps.vpos) as u16;
        if vp as usize >= rows && vp != 261 {
            break;
        }
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
    Captured { dots, trace }
}
