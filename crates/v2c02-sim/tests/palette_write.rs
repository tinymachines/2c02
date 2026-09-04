//! The gate for the palette write-path divergence (docs/p3-report.md):
//! a $2007 palette write with an access width of idle after it must
//! land as written, on a row below and above $20, the way the reference
//! lands it. Under halfphi's `AnyHigh` charge rule the byte lands ORed
//! with the address low byte; the 2C02's netlist declares `AreaVote`,
//! the reference's own rule, and this test holds that declaration.
//! SKIPs by name without the extern; REQUIRE_NETLIST=1 insists.
//! `MUTATE=1` builds the chip under `AnyHigh` and must go red.

use v2c02_sim::harness::Harness;
use v2c02_sim::Ppu;

fn vram(a: u16) -> u8 {
    (((a >> 4) ^ a) & 0xff) as u8
}

fn read_row(h: &mut Harness, lo: u8, n: usize) -> Vec<u8> {
    h.write(6, 0x3f);
    h.wait(48);
    h.write(6, lo);
    h.wait(96);
    (0..n)
        .map(|_| {
            let v = h.read(7);
            h.wait(48);
            v
        })
        .collect()
}

#[test]
fn a_paced_palette_write_lands_as_written() {
    if !v2c02_netlist::available() {
        if std::env::var("REQUIRE_NETLIST").is_ok() {
            panic!("REQUIRE_NETLIST set but extern/visual2c02 is not fetched");
        }
        eprintln!("SKIP: extern/visual2c02 not fetched");
        return;
    }
    let rule = if std::env::var("MUTATE").is_ok_and(|v| v == "1") {
        halfphi::ChargeRule::AnyHigh
    } else {
        halfphi::ChargeRule::AreaVote
    };
    let mut h = Harness::new(Ppu::power_on_with(v2c02_netlist::netlist_with_charge_rule(rule)), vram);
    h.wait(712_100);
    h.read(2);
    h.write(0, 0);
    h.write(1, 0);
    let values = [0x21u8, 0x11, 0x01, 0x2a];
    for row in [0x11u8, 0x21] {
        h.write(6, 0x3f);
        h.wait(48);
        h.write(6, row);
        h.wait(96);
        for &v in &values {
            h.write(7, v);
            h.wait(24);
        }
        h.wait(192);
        let held = read_row(&mut h, row, 4);
        assert_eq!(
            held,
            values.to_vec(),
            "paced $2007 writes to $3F{row:02x}.. under {rule:?} (the reference holds them as written)"
        );
    }
    let s = h.ppu.engine.stats();
    eprintln!("paced palette writes land as written on both rows; area_vote_lows {}", s.area_vote_lows);
    assert!(s.area_vote_lows > 0, "the area vote must have decided at least one group differently from AnyHigh");
}
