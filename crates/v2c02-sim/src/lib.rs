//! The 2C02 as a running machine: halfphi's engine plus the chip's own
//! clock, reset and input pins, initialized exactly the way the
//! reference simulator's initChip does it, so the golden comparison is
//! a comparison and not a coincidence.
//!
//! One deliberate state fix-up, absent from the 6502 layer because the
//! 6502 never needed it: the 2C02 has 38 transistors gated by the
//! supply rail. In silicon they conduct permanently; the reference's
//! init turns them on and nothing ever turns them off (rails are never
//! recalculated); halfphi's power-on state starts every transistor off
//! and, for the same reason, would leave them off forever. `power_on`
//! sets exactly those 38 conducting once, which is both what the
//! silicon does and what the reference does. The 46 ground-gated
//! transistors are permanently off in both models and need nothing.

pub mod harness;

use std::sync::Arc;

use halfphi::{Engine, Netlist, NodeId};

pub struct Sig {
    pub clk0: NodeId,
    pub res: NodeId,
    pub io_ce: NodeId,
    pub int: NodeId,
}

pub struct Ppu {
    pub engine: Engine,
    pub sig: Sig,
}

impl Ppu {
    /// Power on and run the reference's reset recipe: everything to the
    /// power-on state, supply-gated transistors conducting, layout
    /// pulls restored, then (the reference's initChip, statement for
    /// statement) reset and clock held low, chip-enable and int held
    /// high, a full settle, four clock cycles under reset, reset
    /// released.
    pub fn power_on() -> Ppu {
        let nl = v2c02_netlist::netlist();
        Ppu::power_on_with(nl)
    }

    pub fn power_on_with(nl: Arc<Netlist>) -> Ppu {
        let sig = Sig {
            clk0: nl.node("clk0").expect("clk0"),
            res: nl.node("res").expect("res"),
            io_ce: nl.node("io_ce").expect("io_ce"),
            int: nl.node("int").expect("int"),
        };
        let supply_gated: Vec<_> = nl.gates_of(nl.vcc()).to_vec();
        let mut engine = Engine::new(nl);
        engine.force_power_on_state();
        for t in supply_gated {
            engine.state_mut().trans_on.set(t as usize);
        }
        engine.restore_layout_pulls();
        engine.drive_low(sig.res);
        engine.drive_low(sig.clk0);
        engine.drive_high(sig.io_ce);
        engine.drive_high(sig.int);
        engine.settle_all();
        for _ in 0..4 {
            engine.drive_high(sig.clk0);
            engine.drive_low(sig.clk0);
        }
        engine.drive_high(sig.res);
        Ppu { engine, sig }
    }

    /// One half step: toggle the master clock, settle.
    pub fn half_step(&mut self) {
        if self.engine.is_high(self.sig.clk0) {
            self.engine.drive_low(self.sig.clk0);
        } else {
            self.engine.drive_high(self.sig.clk0);
        }
    }

    /// Every node's level as a '0'/'1' line over node ids 0..node_count,
    /// nonexistent ids as '0': byte for byte what the golden generator
    /// writes.
    pub fn state_line(&self) -> String {
        let nl = self.engine.netlist();
        (0..nl.node_count() as NodeId)
            .map(|n| {
                if nl.exists(n) && self.engine.is_high(n) {
                    '1'
                } else {
                    '0'
                }
            })
            .collect()
    }
}
