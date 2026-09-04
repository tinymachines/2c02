//! The P3 step-2 golden: a visible frame of the sprite world off rung 0,
//! with the palette RAM and OAM read back out of the chip beside it, so
//! the stepper renders from what the chip holds rather than from what
//! the world wrote. Writes goldens/p3-sprites.bin (32 palette bytes,
//! 256 OAM bytes, then the 262 x 341 colour bytes) and a stamp.
//! Run with --release; about a minute.

use v2c02_dots::{capture, read_back_palette_and_oam, sprite_world};

fn main() {
    let mut h = sprite_world();
    eprintln!("sprite world ready at half-step {}", h.half_steps);
    let (pal, oam) = read_back_palette_and_oam(&mut h);
    eprintln!(
        "palette as held: {}",
        pal.iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" ")
    );
    let loaded = oam.chunks_exact(4).filter(|s| s[0] != 0xf0).count();
    eprintln!("OAM as held: {loaded} sprites not parked");
    let cap = capture(&mut h, 240);
    eprintln!(
        "captured a frame at half-step {}; spr0_hit rose at {:?}, spr_overflow at {:?}",
        h.half_steps, cap.spr0_hit, cap.spr_overflow
    );

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../goldens");
    std::fs::create_dir_all(&out).unwrap();
    let flag = |f: Option<(u16, u16)>| -> [u8; 4] {
        let (v, hp) = f.unwrap_or((0xffff, 0xffff));
        [v as u8, (v >> 8) as u8, hp as u8, (hp >> 8) as u8]
    };
    let mut bytes = Vec::with_capacity(32 + 256 + 8 + cap.dots.colour.len());
    bytes.extend_from_slice(&pal);
    bytes.extend_from_slice(&oam);
    bytes.extend_from_slice(&flag(cap.spr0_hit));
    bytes.extend_from_slice(&flag(cap.spr_overflow));
    bytes.extend_from_slice(&cap.dots.colour);
    std::fs::write(out.join("p3-sprites.bin"), &bytes).unwrap();
    std::fs::write(
        out.join("p3-sprites.stamp.txt"),
        format!(
            "2c02 P3 sprite golden: one visible frame of the sprite world off rung 0\n\
             layout: palette RAM as held (32), OAM as held (256), spr0_hit first rise (vpos u16, hpos u16),\n\
             spr_overflow first rise (same; ffff = never), colour per dot (262 x 341)\n\
             world: v2c02_dots::sprite_world (standard world + SPRITE_PALETTE + sprite_oam, mask $1E)\n\
             recorded: 2026-09-03 by examples/p3-sprites-golden.rs at {} half-steps\n",
            h.half_steps
        ),
    )
    .unwrap();
    println!("wrote goldens/p3-sprites.bin ({} bytes)", bytes.len());
}
