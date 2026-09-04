//! The 2C02 ladder: a per-dot stepper. The SEQUENCER is a table
//! measured out of the switch-level chip at build time (one event word
//! per dot of a frame; `events.rs` says what each bit is and where it
//! was read from), and the palette RAM the stepper renders the standard
//! world with is a second build-time measurement, read back out of the
//! chip. The DATAPATH below is authored from the proven model and
//! labelled as such. Held to rung 0's dot goldens and to the frame
//! period in `tests/`; the design and the measured positions it was
//! written from are in `docs/p3-plan.md` and `docs/p3-report.md`.
//!
//! Step 1 is the background; step 2 sprites (evaluation, fetch and the
//! priority mux); step 3 the register file, so a world is a register
//! program rather than fields set by hand, and writes land mid-frame at
//! their dots.

use nes_bus::{DotFrame, FrameParity, ACTIVE_DOTS, ACTIVE_ROWS, DOTS_PER_LINE, LINES};

include!("events.rs");

/// The table as recorded by build.rs: little-endian `u16` per dot in
/// (line, dot) order. Empty when the extern was not fetched.
pub static TABLE_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/table.bin"));

/// The palette RAM as the chip holds it after the standard world's
/// register program, read back through $2007 (paced) by build.rs.
/// Empty when the extern was not fetched.
pub static PALETTE_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/palette.bin"));

/// True when build.rs could run the chip and both measurements are real.
pub fn table_available() -> bool {
    TABLE_BYTES.len() == LINES * DOTS_PER_LINE * 2 && PALETTE_BYTES.len() == 32
}

/// The recorded table, decoded.
pub fn table() -> Vec<u16> {
    assert!(table_available(), "v2c02-fast: no table (extern/visual2c02 not fetched at build time)");
    TABLE_BYTES.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect()
}

/// The standard world's palette RAM as measured. Entry 0 is the
/// backdrop the chip actually renders with.
pub fn palette_as_loaded() -> [u8; 32] {
    assert!(table_available(), "v2c02-fast: no palette (extern/visual2c02 not fetched at build time)");
    let mut p = [0u8; 32];
    p.copy_from_slice(PALETTE_BYTES);
    p
}

/// A register write inside a frame, applied on the first half-step of
/// dot (vpos, hpos) of the stepper's traversal.
#[derive(Clone, Copy, Debug)]
pub struct DotWrite {
    pub vpos: usize,
    pub hpos: usize,
    pub reg: u8,
    pub val: u8,
}

/// One of the eight sprite units: the pattern row fetched for the next
/// line, where it starts, and how it composes.
#[derive(Clone, Copy, Default)]
struct Unit {
    lo: u8,
    hi: u8,
    x: u8,
    palette: u8,
    behind: bool,
    hflip: bool,
    /// This unit carries OAM sprite 0 (for the hit).
    is_zero: bool,
    active: bool,
}

/// The authored datapath. Names follow the wiki's register model (v, t,
/// fine x, w) because that model is what P1 and P2 proved this chip
/// implements; the POSITIONS at which anything happens come only from
/// the table.
pub struct Fast {
    table: Vec<u16>,
    /// Per dot: the background shifters present a pixel and advance.
    /// Derived from the table: the eight dots ending at each INC_X.
    active: Vec<bool>,
    vram: fn(u16) -> u8,
    /// The 32-byte palette RAM.
    pub palette: [u8; 32],
    /// The 256-byte OAM.
    pub oam: [u8; 256],
    /// $2000 and $2001 as last written. 8x16 sprites (bit 5 of $2000)
    /// and left-edge clipping (bits 1, 2 of $2001) are not modelled.
    pub ctrl: u8,
    pub mask: u8,
    pub v: u16,
    pub t: u16,
    pub fine_x: u8,
    /// The first/second write toggle shared by $2005 and $2006.
    pub w: bool,
    pub oamaddr: u8,
    read_buffer: u8,
    /// $2007 writes outside the palette, captured rather than stored:
    /// the worlds' VRAM is a pure function, on the chip and here.
    pub vram_writes: Vec<(u16, u8)>,
    nt: u8,
    at: u8,
    pt_lo: u8,
    pt_hi: u8,
    shift_lo: u16,
    shift_hi: u16,
    attr_lo: u16,
    attr_hi: u16,
    /// The secondary OAM the evaluation selected for the next line, and
    /// the units the sprite window fetched from it.
    sec: [(u8, u8, u8, u8, bool); 8],
    sec_n: usize,
    fetched: usize,
    units: [Unit; 8],
    pub vbl: bool,
    /// The first sprite-0 hit of the frame, as (line, pixel), and the
    /// evaluation overflow (a ninth sprite in range on some line).
    pub spr0_hit: Option<(usize, usize)>,
    pub spr_overflow: bool,
}

impl Fast {
    /// A stepper over the recorded table and the standard world's
    /// measured palette, in the given world's CHR space.
    pub fn new(vram: fn(u16) -> u8) -> Fast {
        Fast::with_table(table(), vram, palette_as_loaded())
    }

    /// The same with a caller-supplied table and palette: the mutation
    /// proofs drop events from a copy and must go red.
    pub fn with_table(table: Vec<u16>, vram: fn(u16) -> u8, palette: [u8; 32]) -> Fast {
        assert_eq!(table.len(), LINES * DOTS_PER_LINE);
        // The shifters run on the eight dots that end at each INC_X: the
        // tile's four fetches and their reads. Measured positions, not a
        // typed range.
        let mut active = vec![false; table.len()];
        for (i, &e) in table.iter().enumerate() {
            if e & INC_X != 0 {
                let line_start = (i / DOTS_PER_LINE) * DOTS_PER_LINE;
                active[i.saturating_sub(7).max(line_start)..=i].fill(true);
            }
        }
        Fast {
            table,
            active,
            vram,
            palette,
            oam: [0xff; 256],
            ctrl: 0,
            mask: 0,
            v: 0,
            t: 0,
            fine_x: 0,
            w: false,
            oamaddr: 0,
            read_buffer: 0,
            vram_writes: Vec::new(),
            nt: 0,
            at: 0,
            pt_lo: 0,
            pt_hi: 0,
            shift_lo: 0,
            shift_hi: 0,
            attr_lo: 0,
            attr_hi: 0,
            sec: [(0xff, 0xff, 0xff, 0xff, false); 8],
            sec_n: 0,
            fetched: 0,
            units: [Unit::default(); 8],
            vbl: false,
            spr0_hit: None,
            spr_overflow: false,
        }
    }

    // The register file, authored from the model the chip was proved to
    // implement. Each write is what the chip does with the byte; WHEN a
    // mid-frame write takes effect relative to its access is the
    // scroll golden's fit, not something typed here.

    /// A CPU write to $2000 + reg.
    pub fn write(&mut self, reg: u8, val: u8) {
        match reg & 7 {
            0 => {
                self.ctrl = val;
                self.t = (self.t & !0x0c00) | (((val & 3) as u16) << 10);
            }
            1 => self.mask = val,
            3 => self.oamaddr = val,
            4 => {
                self.oam[self.oamaddr as usize] = val;
                self.oamaddr = self.oamaddr.wrapping_add(1);
            }
            5 => {
                if !self.w {
                    self.t = (self.t & !0x001f) | (val >> 3) as u16;
                    self.fine_x = val & 7;
                } else {
                    self.t = (self.t & !0x73e0) | (((val & 7) as u16) << 12) | (((val >> 3) as u16) << 5);
                }
                self.w = !self.w;
            }
            6 => {
                if !self.w {
                    self.t = (self.t & 0x00ff) | (((val & 0x3f) as u16) << 8);
                } else {
                    self.t = (self.t & 0xff00) | val as u16;
                    self.v = self.t;
                }
                self.w = !self.w;
            }
            7 => {
                let a = self.v & 0x3fff;
                if a >= 0x3f00 {
                    self.palette[Self::palette_index(a)] = val & 0x3f;
                } else {
                    self.vram_writes.push((a, val));
                }
                self.v = self.v.wrapping_add(self.increment()) & 0x7fff;
            }
            _ => {}
        }
    }

    /// A CPU read of $2000 + reg.
    pub fn read(&mut self, reg: u8) -> u8 {
        match reg & 7 {
            2 => {
                let s = ((self.vbl as u8) << 7) | ((self.spr0_hit.is_some() as u8) << 6) | ((self.spr_overflow as u8) << 5);
                self.vbl = false;
                self.w = false;
                s
            }
            4 => self.oam[self.oamaddr as usize],
            7 => {
                let a = self.v & 0x3fff;
                let out = if a >= 0x3f00 {
                    self.read_buffer = self.read_vram(a & 0x2fff);
                    self.palette[Self::palette_index(a)]
                } else {
                    let out = self.read_buffer;
                    self.read_buffer = self.read_vram(a);
                    out
                };
                self.v = self.v.wrapping_add(self.increment()) & 0x7fff;
                out
            }
            _ => 0,
        }
    }

    /// Run a register program, write after write, as a world does
    /// between frames.
    pub fn run_program(&mut self, program: &[(u8, u8)]) {
        for &(reg, val) in program {
            self.write(reg, val);
        }
    }

    #[inline]
    fn increment(&self) -> u16 {
        if self.ctrl & 4 != 0 {
            32
        } else {
            1
        }
    }

    /// $3F00..$3F1F, with $3F10/$14/$18/$1C mirroring $3F00/$04/$08/$0C.
    #[inline]
    fn palette_index(a: u16) -> usize {
        let i = (a & 0x1f) as usize;
        if i & 0x13 == 0x10 {
            i & !0x10
        } else {
            i
        }
    }

    #[inline]
    fn read_vram(&self, addr: u16) -> u8 {
        (self.vram)(addr & 0x3fff)
    }

    #[inline]
    fn inc_x(&mut self) {
        if self.v & 0x001f == 31 {
            self.v &= !0x001f;
            self.v ^= 0x0400;
        } else {
            self.v += 1;
        }
    }

    #[inline]
    fn inc_y(&mut self) {
        if self.v & 0x7000 != 0x7000 {
            self.v += 0x1000;
        } else {
            self.v &= !0x7000;
            let mut y = (self.v >> 5) & 0x1f;
            if y == 29 {
                y = 0;
                self.v ^= 0x0800;
            } else if y == 31 {
                y = 0;
            } else {
                y += 1;
            }
            self.v = (self.v & !0x03e0) | (y << 5);
        }
    }

    /// The background's palette index for the shifters' current bit:
    /// pattern from the two planes, attribute above it, and a
    /// transparent pattern folded to index 0 (the backdrop). The fold is
    /// measured: `pal_ptr` reads 0 wherever `pixel_color` has a zero
    /// pattern, whatever the attribute bits say.
    #[inline]
    fn bg_pixel(&self) -> u8 {
        let sel = 15 - self.fine_x as u32;
        let p = (((self.shift_hi >> sel) & 1) << 1 | ((self.shift_lo >> sel) & 1)) as u8;
        if p == 0 {
            return 0;
        }
        let a = (((self.attr_hi >> sel) & 1) << 1 | ((self.attr_lo >> sel) & 1)) as u8;
        (a << 2) | p
    }

    /// The sprite side of the mux for pixel `x` of the line being drawn:
    /// the first unit in slot order with an opaque pattern bit under x.
    /// Returns (palette index in the sprite half, behind, is sprite 0).
    #[inline]
    fn spr_pixel(&self, x: usize) -> Option<(u8, bool, bool)> {
        for u in self.units.iter().filter(|u| u.active) {
            let ux = u.x as usize;
            if x < ux || x >= ux + 8 {
                continue;
            }
            let col = (x - ux) as u32;
            let bit = if u.hflip { col } else { 7 - col };
            let p = ((u.hi >> bit) & 1) << 1 | ((u.lo >> bit) & 1);
            if p != 0 {
                return Some((0x10 | (u.palette << 2) | p, u.behind, u.is_zero));
            }
        }
        None
    }

    /// Evaluation for the line after `line`: the sprites whose eight
    /// rows cover it, in OAM order, the first eight kept. Authored as one
    /// step at the sprite window; the chip spreads it over dots 65..256
    /// (measured), which matters for the flags' timing, not the picture.
    fn evaluate(&mut self, line: usize) {
        self.sec_n = 0;
        for i in 0..64 {
            let y = self.oam[i * 4] as usize;
            if line < y || line - y >= 8 {
                continue;
            }
            if self.sec_n == 8 {
                self.spr_overflow = true;
                break;
            }
            self.sec[self.sec_n] = (self.oam[i * 4], self.oam[i * 4 + 1], self.oam[i * 4 + 2], self.oam[i * 4 + 3], i == 0);
            self.sec_n += 1;
        }
    }

    #[inline]
    fn colour(&self, index: u8) -> u8 {
        self.palette[index as usize & 0x1f] & 0x3f
    }

    /// Render one frame from the current register state.
    pub fn frame(&mut self) -> DotFrame {
        self.frame_with_writes(&[])
    }

    /// Render one frame with register writes applied at their dots.
    /// Pixel x of row y lands at the contract's dot x + 1 (active from
    /// dot 1); rows and dots the stepper does not produce keep the
    /// backdrop. What the chip's own output pipeline adds on top of this
    /// (pixel x reaches `pal_d` three dots later) is the golden's
    /// convention and lives in the gates that compare against it, not
    /// here. `writes` must be in traversal order (the pre-render line
    /// first, then lines 0..).
    pub fn frame_with_writes(&mut self, writes: &[DotWrite]) -> DotFrame {
        assert!(self.ctrl & 0x20 == 0, "8x16 sprites are not modelled");
        let show_sprites = self.mask & 0x10 != 0;
        let backdrop = self.colour(0);
        let mut out = DotFrame::filled(FrameParity::Even, backdrop, 0);
        let mut line = [0u8; DOTS_PER_LINE];
        self.spr0_hit = None;
        self.spr_overflow = false;
        let mut next_write = 0usize;
        // The picture's frame begins at the pre-render line: its vertical
        // copy sets v for row 0 and its dots 321..336 prefetch row 0's
        // first two tiles, so the stepper runs line 261 first, then 0
        // through 260. The table is in frame order; only the traversal
        // starts a line early.
        let order = std::iter::once(LINES - 1).chain(0..LINES - 1);
        for vp in order {
            let base = vp * DOTS_PER_LINE;
            let render_line = vp < ACTIVE_ROWS || vp == LINES - 1;
            self.fetched = 0;
            let mut evaluated = false;
            for (hp, slot) in line.iter_mut().enumerate() {
                while next_write < writes.len() && writes[next_write].vpos == vp && writes[next_write].hpos == hp {
                    let w = writes[next_write];
                    self.write(w.reg, w.val);
                    next_write += 1;
                }
                let e = self.table[base + hp];
                if render_line && self.active[base + hp] {
                    let mut index = self.bg_pixel();
                    if show_sprites && vp < ACTIVE_ROWS && (1..=ACTIVE_DOTS).contains(&hp) {
                        let x = hp - 1;
                        if let Some((s, behind, is_zero)) = self.spr_pixel(x) {
                            let bg_opaque = index & 3 != 0;
                            if is_zero && bg_opaque && x != 255 && self.spr0_hit.is_none() {
                                self.spr0_hit = Some((vp, x));
                            }
                            if !(behind && bg_opaque) {
                                index = s;
                            }
                        }
                    }
                    *slot = index;
                    self.shift_lo <<= 1;
                    self.shift_hi <<= 1;
                    self.attr_lo <<= 1;
                    self.attr_hi <<= 1;
                }
                if e & FETCH_NT != 0 {
                    self.nt = self.read_vram(0x2000 | (self.v & 0x0fff));
                }
                if e & FETCH_AT != 0 {
                    let a = 0x23c0 | (self.v & 0x0c00) | ((self.v >> 4) & 0x38) | ((self.v >> 2) & 0x07);
                    let byte = self.read_vram(a);
                    let shift = ((self.v >> 4) & 4) | (self.v & 2);
                    self.at = (byte >> shift) & 3;
                }
                if e & (FETCH_PT_LO | FETCH_PT_HI) != 0 {
                    let bg_hi = (self.ctrl & 0x10 != 0) as u16;
                    let base_addr = (bg_hi << 12) | ((self.nt as u16) << 4) | ((self.v >> 12) & 7);
                    if e & FETCH_PT_LO != 0 {
                        self.pt_lo = self.read_vram(base_addr);
                    }
                    if e & FETCH_PT_HI != 0 {
                        self.pt_hi = self.read_vram(base_addr | 8);
                    }
                }
                if render_line && e & (SPR_GARBAGE | SPR_PT_LO | SPR_PT_HI) != 0 {
                    if !evaluated {
                        self.evaluate(vp);
                        evaluated = true;
                        self.units = [Unit::default(); 8];
                    }
                    if e & SPR_PT_LO != 0 && self.fetched < 8 {
                        let k = self.fetched;
                        self.fetched += 1;
                        if k < self.sec_n {
                            let (y, tile, attr, x, is_zero) = self.sec[k];
                            let mut row = (vp - y as usize) as u16 & 7;
                            if attr & 0x80 != 0 {
                                row = 7 - row;
                            }
                            let spr_hi = (self.ctrl & 0x08 != 0) as u16;
                            let addr = (spr_hi << 12) | ((tile as u16) << 4) | row;
                            self.units[k] = Unit {
                                lo: self.read_vram(addr),
                                hi: self.read_vram(addr | 8),
                                x,
                                palette: attr & 3,
                                behind: attr & 0x20 != 0,
                                hflip: attr & 0x40 != 0,
                                is_zero,
                                active: true,
                            };
                        }
                    }
                }
                if e & INC_X != 0 {
                    self.inc_x();
                    // The tile just fetched enters the low byte of the
                    // shifters as its increment lands; eight shifts later
                    // it is the byte being presented.
                    self.shift_lo = (self.shift_lo & 0xff00) | self.pt_lo as u16;
                    self.shift_hi = (self.shift_hi & 0xff00) | self.pt_hi as u16;
                    self.attr_lo = (self.attr_lo & 0xff00) | if self.at & 1 != 0 { 0xff } else { 0 };
                    self.attr_hi = (self.attr_hi & 0xff00) | if self.at & 2 != 0 { 0xff } else { 0 };
                }
                if e & INC_Y != 0 {
                    self.inc_y();
                }
                if e & COPY_X != 0 {
                    self.v = (self.v & !0x041f) | (self.t & 0x041f);
                }
                if e & COPY_Y != 0 {
                    self.v = (self.v & !0x7be0) | (self.t & 0x7be0);
                }
                if e & SET_VBL != 0 {
                    self.vbl = true;
                }
                if e & CLR_FLAGS != 0 {
                    self.vbl = false;
                }
            }
            if vp < ACTIVE_ROWS {
                for (d, &index) in line.iter().enumerate().skip(1).take(ACTIVE_DOTS) {
                    out.set(vp, d, self.colour(index), 0);
                }
            }
        }
        assert_eq!(next_write, writes.len(), "a scheduled write was never reached (writes must be in traversal order)");
        out
    }
}
