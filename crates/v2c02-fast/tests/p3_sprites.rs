//! The P3 step-2 gate: the stepper with sprites against rung 0's sprite
//! world golden, every visible dot, rendering from the palette RAM and
//! OAM the chip was measured to hold. Sprite 0's hit is checked against
//! P2's measured position. SKIPs by name without the table or
//! `goldens/p3-sprites.bin` (written by `p3-sprites-golden`);
//! `REQUIRE_GOLDEN_P3=1` insists. `MUTATE=1` drops the sprite pattern
//! fetches from the table and must go red.

use nes_bus::{ACTIVE_DOTS, ACTIVE_ROWS, DOTS_PER_LINE, LINES};
use v2c02_dots::{sprite_program, standard_program, vram};
use v2c02_fast::{table, Fast, SPR_PT_HI, SPR_PT_LO};

/// The golden holds pixel x at dot x + 3 (see tests/p3.rs).
const GOLDEN_PIXEL_OFFSET: usize = 3;

struct Golden {
    palette: [u8; 32],
    oam: [u8; 256],
    /// First rise of `spr0_hit` and `spr_overflow` on rung 0, (vpos, hpos).
    spr0_hit: Option<(usize, usize)>,
    spr_overflow: Option<(usize, usize)>,
    dots: Vec<u8>,
}

fn golden() -> Option<Golden> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../goldens/p3-sprites.bin");
    let flag = |b: &[u8]| -> Option<(usize, usize)> {
        let v = u16::from_le_bytes([b[0], b[1]]);
        let h = u16::from_le_bytes([b[2], b[3]]);
        (v != 0xffff).then_some((v as usize, h as usize))
    };
    match std::fs::read(path) {
        Ok(g) if g.len() == 32 + 256 + 8 + LINES * DOTS_PER_LINE => {
            let mut palette = [0u8; 32];
            palette.copy_from_slice(&g[..32]);
            let mut oam = [0u8; 256];
            oam.copy_from_slice(&g[32..288]);
            Some(Golden {
                palette,
                oam,
                spr0_hit: flag(&g[288..292]),
                spr_overflow: flag(&g[292..296]),
                dots: g[296..].to_vec(),
            })
        }
        _ => {
            if std::env::var("REQUIRE_GOLDEN_P3").is_ok() {
                panic!("REQUIRE_GOLDEN_P3 set but goldens/p3-sprites.bin is missing");
            }
            None
        }
    }
}

#[test]
fn the_stepper_with_sprites_matches_rung_0_on_every_visible_dot() {
    if !v2c02_fast::table_available() {
        eprintln!("SKIP: no table (extern/visual2c02 not fetched at build time)");
        return;
    }
    let Some(g) = golden() else {
        eprintln!("SKIP: no goldens/p3-sprites.bin");
        return;
    };
    let mut t = table();
    if std::env::var("MUTATE").is_ok_and(|v| v == "1") {
        // Drop data: no sprite pattern fetch, no sprite pixels.
        t.iter_mut().for_each(|e| *e &= !(SPR_PT_LO | SPR_PT_HI));
    }
    let mut f = Fast::with_table(t, vram, g.palette);
    // The sprite world as a register program through the register file:
    // the standard world's, then the sprite world's. The OAM the file
    // derives from the $2003/$2004 writes must be the OAM the chip holds
    // (read back beside the golden), byte 2's unimplemented bits masked
    // as P2 measured; the palette is rendered from what the chip holds
    // (docs/p3-report.md, the write-path question).
    f.read(2);
    f.run_program(&standard_program());
    for (reg, val, _) in sprite_program() {
        f.write(reg, val);
    }
    let derived: Vec<u8> = f.oam.iter().enumerate().map(|(i, &b)| if i % 4 == 2 { b & 0xe3 } else { b }).collect();
    assert_eq!(derived, g.oam.to_vec(), "OAM through the register file against the chip's read-back");
    assert_eq!((f.mask, f.ctrl, f.t, f.fine_x), (0x1e, 0, 0, 0), "the register file after the sprite program");
    // The sprite palettes are written paced and land as written (halfphi
    // 0.1.6), so the register file's derivation of them is held to the
    // chip's read-back, entry for entry. The background half is the
    // standard world's back-to-back program, which both engines land
    // one entry early (docs/p3-report.md), and stays read back.
    assert_eq!(&f.palette[16..], &g.palette[16..], "sprite palettes through the register file against the chip's read-back");
    f.palette = g.palette;
    let frame = f.frame();

    let mut bad = Vec::new();
    for r in 0..ACTIVE_ROWS {
        for d in 1..=ACTIVE_DOTS {
            let gd = g.dots[r * DOTS_PER_LINE + d + GOLDEN_PIXEL_OFFSET];
            let c = frame.at(r, d).0;
            if gd != c {
                bad.push((r, d, gd, c));
            }
        }
    }
    if !bad.is_empty() {
        for (r, d, gd, c) in bad.iter().take(16) {
            eprintln!("row {r} pixel {}: golden {gd:02x} stepper {c:02x}", d - 1);
        }
        panic!("{} of {} visible dots disagree with rung 0", bad.len(), ACTIVE_ROWS * ACTIVE_DOTS);
    }
    // Sprite 0's hit, held to where rung 0's `spr0_hit` rose in this
    // world: pixel x reaches the mux two dots after its position
    // (measured, docs/p3-report.md), so the chip's hpos is the stepper's
    // pixel plus 2. The overflow is held as a fact here; its dot is
    // recorded in the golden for the step that models the scan.
    let chip_hit = g.spr0_hit.map(|(v, hp)| (v, hp - 2));
    assert_eq!(f.spr0_hit, chip_hit, "sprite 0 hit (line, pixel) against rung 0's spr0_hit rise");
    assert_eq!(f.spr_overflow, g.spr_overflow.is_some(), "sprite overflow against rung 0's spr_overflow");
    eprintln!(
        "{} visible dots agree with rung 0 with sprites on; sprite 0 hit at {:?} (chip {:?}), overflow {:?}",
        ACTIVE_ROWS * ACTIVE_DOTS,
        f.spr0_hit,
        g.spr0_hit,
        g.spr_overflow
    );
}
