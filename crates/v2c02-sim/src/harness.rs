//! The PPU's world: the CPU-side register protocol and the CHR/VRAM
//! bus, each mirrored statement for statement from the reference
//! simulator's own machinery (cpucmd.js: "operate it the way a RP2A03
//! would", 24 clock edges per access with the address at edge 24, chip
//! enable at 16, release at 1; macros.js handleChrBus: address latched
//! on ALE's rising edge, data driven on /RD's falling edge and floated
//! on its rising edge, writes captured on /WR's rising edge). The
//! golden generator scripts the same protocol in JS, so the node-level
//! comparison covers the harness as well as the chip.

use halfphi::NodeId;
use nes_bus::pins::PpuPins;

use crate::Ppu;

pub struct Harness {
    pub ppu: Ppu,
    io_ab: [NodeId; 3],
    io_db: [NodeId; 8],
    io_rw: NodeId,
    ab: [NodeId; 14],
    db: [NodeId; 8],
    ale: NodeId,
    rd: NodeId,
    wr: NodeId,
    ext_out_n: [NodeId; 4],
    /// Test-only (the N0 contract gate): route the CHR bus through
    /// `PpuPins::mutated_rd_for_proof`, which must send the P1 golden
    /// red. Setting this anywhere but a mutation proof is a bug by name.
    pub mutate_rd_for_proof: bool,
    chr_ale: bool,
    chr_rd: bool,
    chr_wr: bool,
    chr_addr: u16,
    /// The CHR ROM/RAM as a pure function of address; writes are
    /// captured rather than stored, because P1's world is read-only.
    pub vram: fn(u16) -> u8,
    pub vram_writes: Vec<(u16, u8)>,
    pub half_steps: u64,
}

impl Harness {
    pub fn new(ppu: Ppu, vram: fn(u16) -> u8) -> Harness {
        let nl = ppu.engine.netlist().clone();
        let n = |name: &str| nl.node(name).unwrap_or_else(|| panic!("node {name}"));
        let arr = |prefix: &str, i: usize| n(&format!("{prefix}{i}"));
        Harness {
            io_ab: std::array::from_fn(|i| arr("io_ab", i)),
            io_db: std::array::from_fn(|i| arr("io_db", i)),
            io_rw: n("io_rw"),
            ab: std::array::from_fn(|i| arr("ab", i)),
            db: std::array::from_fn(|i| arr("db", i)),
            ale: n("ale"),
            rd: n("rd"),
            wr: n("wr"),
            ext_out_n: std::array::from_fn(|i| arr("/ext_out", i)),
            mutate_rd_for_proof: false,
            chr_ale: ppu.engine.is_high(n("ale")),
            chr_rd: ppu.engine.is_high(n("rd")),
            chr_wr: ppu.engine.is_high(n("wr")),
            chr_addr: 0,
            vram,
            vram_writes: Vec::new(),
            half_steps: 0,
            ppu,
        }
    }

    /// writeBits: set every pull, then one settle with all the pins as
    /// seeds; per-bit settling would let the chip see a half-updated
    /// bus (halfphi's own set_pull doc says the same).
    fn write_bits(&mut self, nodes: &[NodeId], mut val: u32) {
        for &n in nodes {
            self.ppu.engine.set_pull(n, val & 1 != 0);
            val >>= 1;
        }
        self.ppu.engine.settle(nodes);
    }

    fn float_bits(&mut self, nodes: &[NodeId]) {
        for &n in nodes {
            let e = self.ppu.engine.state_mut();
            e.pullup.clear(n as usize);
            e.pulldown.clear(n as usize);
        }
        self.ppu.engine.settle(nodes);
    }

    fn read_bits(&self, nodes: &[NodeId]) -> u32 {
        nodes
            .iter()
            .enumerate()
            .map(|(i, &n)| (self.ppu.engine.is_high(n) as u32) << i)
            .sum()
    }

    /// One half step of the whole world: the clock toggles, then the
    /// CHR bus reacts to the edges the new state shows, exactly the
    /// reference's halfStep order (clock, then handleChrBus).
    pub fn half_step(&mut self) {
        self.ppu.half_step();
        self.handle_chr_bus();
        self.half_steps += 1;
    }

    /// The chip-driven view of the package pins, this half-step, as the
    /// console contract's frame. Two extraction notes, both faithful to
    /// the reference's own model of the bus:
    ///
    /// - `ad` and `a_hi` are read from the internal address bus, which is
    ///   what the pads carry in the address phase (the AD0..7 data phase
    ///   is serviced by the harness as bus master, not observed here; the
    ///   full pad mux is the console's problem, at N5).
    /// - `ext` is the EXT0..3 pins as driven by the chip's inverted
    ///   output drivers (`/ext_outN`); nothing external drives them in
    ///   this world.
    /// - `vout` is `None`: a digital extraction does not sample the DAC.
    ///   The sample-exact comparison in v2c02-dots owns that pin.
    pub fn pins(&self) -> PpuPins {
        let e = &self.ppu.engine;
        let bits = |ns: &[NodeId]| -> u32 {
            ns.iter()
                .enumerate()
                .map(|(i, &n)| (e.is_high(n) as u32) << i)
                .sum()
        };
        PpuPins {
            clk: e.is_high(self.ppu.sig.clk0),
            cpu_rw: e.is_high(self.io_rw),
            cpu_d: bits(&self.io_db) as u8,
            cpu_a: bits(&self.io_ab) as u8,
            cs_n: e.is_high(self.ppu.sig.io_ce),
            ext: (!bits(&self.ext_out_n) & 0xf) as u8,
            int_n: e.is_high(self.ppu.sig.int),
            rst_n: e.is_high(self.ppu.sig.res),
            vout: None,
            ale: e.is_high(self.ale),
            ad: (bits(&self.ab) & 0xff) as u8,
            a_hi: (bits(&self.ab) >> 8) as u8,
            rd_n: e.is_high(self.rd),
            wr_n: e.is_high(self.wr),
        }
    }

    fn handle_chr_bus(&mut self) {
        let mut p = self.pins();
        if self.mutate_rd_for_proof {
            p = p.mutated_rd_for_proof();
        }
        let (ale, rd, wr) = (p.ale, p.rd_n, p.wr_n);
        if !self.chr_ale && ale {
            self.chr_addr = (p.ad as u16) | ((p.a_hi as u16) << 8);
        }
        if self.chr_rd && !rd {
            let d = (self.vram)(self.chr_addr & 0x3fff);
            self.write_bits(&self.db.clone(), d as u32);
        }
        if !self.chr_rd && rd {
            self.float_bits(&self.db.clone());
        }
        if !self.chr_wr && wr {
            let d = self.read_bits(&self.db) as u8;
            self.vram_writes.push((self.chr_addr & 0x3fff, d));
        }
        self.chr_ale = ale;
        self.chr_rd = rd;
        self.chr_wr = wr;
    }

    /// The pin actions for one edge of the reference's 24-edge access
    /// protocol, applied BEFORE that edge's half-step (the reference
    /// runs its cpucmd handler at the top of halfStep). Returns the
    /// byte sampled at edge 1 of a read.
    pub fn access_edge(&mut self, rw: bool, reg: u8, val: u8, counter: u32) -> Option<u8> {
        let mut sampled = None;
        if counter == 24 {
            self.write_bits(&self.io_ab.clone(), reg as u32);
            if rw {
                self.float_bits(&self.io_db.clone());
            } else {
                self.write_bits(&self.io_db.clone(), val as u32);
            }
            let n = self.io_rw;
            if rw {
                self.ppu.engine.drive_high(n);
            } else {
                self.ppu.engine.drive_low(n);
            }
        }
        if counter == 16 {
            let n = self.ppu.sig.io_ce;
            self.ppu.engine.drive_low(n);
        }
        if counter == 1 {
            if rw {
                sampled = Some(self.read_bits(&self.io_db) as u8);
            }
            let n = self.ppu.sig.io_ce;
            self.ppu.engine.drive_high(n);
        }
        sampled
    }

    /// The reference floats the data bus when the next command starts;
    /// the harness does it at the end of each access.
    pub fn end_access(&mut self) {
        self.float_bits(&self.io_db.clone());
    }

    /// One CPU access, the reference's 24-edge protocol. `reg` is the
    /// register number ($2000 + reg), `rw` true for a read. Returns the
    /// byte sampled at edge 1 for reads.
    pub fn cpu_access(&mut self, rw: bool, reg: u8, val: u8) -> u8 {
        let mut sampled = 0u8;
        for counter in (1..=24u32).rev() {
            if let Some(d) = self.access_edge(rw, reg, val, counter) {
                sampled = d;
            }
            self.half_step();
        }
        self.end_access();
        sampled
    }

    pub fn write(&mut self, reg: u8, val: u8) {
        self.cpu_access(false, reg, val);
    }

    pub fn read(&mut self, reg: u8) -> u8 {
        self.cpu_access(true, reg, 0)
    }

    pub fn wait(&mut self, half_steps: u64) {
        for _ in 0..half_steps {
            self.half_step();
        }
    }
}
