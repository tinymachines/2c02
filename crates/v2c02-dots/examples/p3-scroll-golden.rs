//! The P3 step-3 golden: a visible frame of the scroll world off rung 0
//! with the mid-frame writes performed inside the frame, the palette RAM
//! read back beside it. Writes goldens/p3-scroll.bin (32 palette bytes,
//! then the 262 x 341 colour bytes) and a stamp naming the writes and
//! the half-step each access started at. Run with --release.

use v2c02_dots::{capture_with_writes, read_back_palette_and_oam, scroll_world, SCROLL_PROGRAM, SCROLL_WRITES};

fn main() {
    let mut h = scroll_world();
    eprintln!("scroll world ready at half-step {}", h.half_steps);
    let (pal, _) = read_back_palette_and_oam(&mut h);
    // The read-back moved t and v; restore the scroll the world set.
    for (reg, val, idle) in SCROLL_PROGRAM {
        h.write(reg, val);
        h.wait(idle);
    }
    eprintln!("palette as held: {}", pal.iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" "));
    let (cap, started) = capture_with_writes(&mut h, 240, &SCROLL_WRITES);
    eprintln!("captured a frame at half-step {}; accesses started at {:?}", h.half_steps, started);

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../goldens");
    std::fs::create_dir_all(&out).unwrap();
    let mut bytes = Vec::with_capacity(32 + cap.dots.colour.len());
    bytes.extend_from_slice(&pal);
    bytes.extend_from_slice(&cap.dots.colour);
    std::fs::write(out.join("p3-scroll.bin"), &bytes).unwrap();
    let writes: Vec<String> = SCROLL_WRITES
        .iter()
        .zip(&started)
        .map(|(w, s)| format!("({},{}) ${:04x} <- {:02x} at half-step {s}", w.vpos, w.hpos, 0x2000 + w.reg as u16, w.val))
        .collect();
    std::fs::write(
        out.join("p3-scroll.stamp.txt"),
        format!(
            "2c02 P3 scroll golden: one visible frame of the scroll world off rung 0\n\
             layout: palette RAM as held (32), colour per dot (262 x 341)\n\
             world: v2c02_dots::scroll_world (SCROLL_PROGRAM), mid-frame SCROLL_WRITES:\n  {}\n\
             recorded: 2026-09-04 by examples/p3-scroll-golden.rs at {} half-steps\n",
            writes.join("\n  "),
            h.half_steps
        ),
    )
    .unwrap();
    println!("wrote goldens/p3-scroll.bin ({} bytes)", bytes.len());
}
