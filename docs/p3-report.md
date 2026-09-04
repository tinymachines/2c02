# P3 report, step 1: the per-dot stepper, measured before it was built

`docs/p3-plan.md` set the design and the order: measure a per-dot
table-driven stepper's frame time first, and build the bit-sliced
datapath only on a recorded shortfall. This is what was measured.
`crates/v2c02-fast` is the stepper; `cargo test --release -p v2c02-fast
--test p3` is the gate, `REQUIRE_GOLDEN_P3=1` insists, `MUTATE=1` must
go red.

## The gate

- **Dot golden.** The stepper's frame agrees with rung 0's
  (`goldens/p1-dots.bin`, the P1 capture of the standard world) on
  **every visible dot: 62,160 per frame**, the 256 pixels and the three
  lead-in dots of each of 240 rows, no exemption. The fit that pinned
  the golden offset (`examples/p3-fit.rs`) is sharp: offset 3 gives 0
  mismatches, offsets 2 and 4 give about 39,000 each.
- **Frame time.** The period is derived, 714,736 master half-steps at
  12 x f_sc (ntsc-grid's 315/88 MHz), 16.639 ms. Over 500 frames in a
  release build (`examples/p3-bench.rs`): **mean 1.004 ms, 996 frames
  per second, 16.6x inside the period; worst 1.781 ms, 9.3x inside.**
  The test's own 200-frame run saw a worst of 3.110 ms (5.4x inside),
  the spread being the machine, not the stepper. There is no shortfall,
  and step 2 of the plan (the bit-sliced datapath) is not built.
- **`MUTATE=1`** drops INC_X from the table: 34,560 of 62,160 dots
  disagree and the test is red.

## How the stepper is made

The sequencer is a TABLE, one event word per dot of a frame, recorded
by `build.rs` from one frame of the switch-level chip in the standard
world and written to `OUT_DIR` (NC-SA-derived, never committed). Each
bit is a named node high on any half-step of the dot, or a CHR-bus
fetch classified by the address the chip latched at ALE; `src/events.rs`
is the one definition both recorder and stepper include. The shifters'
active window is derived from the table too: the eight dots ending at
each INC_X. The palette RAM the stepper renders with is a second
build-time measurement, read back out of the chip through $2007 with
an access-width of idle between accesses, twice, and the passes must
agree. The datapath (v, t, fine x, the latches and shifters, the
increments and copies, the palette lookup) is authored from the model
P1 and P2 proved and labelled so. The build runs the chip for about a
minute; the workspace sets `build-override` opt-level 3 so the build
script's dependencies are not compiled unoptimised.

## What was measured, in the order it was measured

**The sequencer, before any datapath line** (`examples/p3-fetch-probe.rs`,
per dot with an 8-half-step phase mask per control): nametable,
attribute, pattern-low and pattern-high fetches latched at dots 8k+1,
8k+3, 8k+5, 8k+7 of each tile with the read on the following dot; the
two prefetch tiles at 321..335 and two dummy nametable fetches at 337
and 339; INC_X at 8, 16, ..., 256, 328, 336 (34 per line, 8,194 per
frame); INC_Y at 256; the horizontal copy at 257 on every rendered line;
the vertical copy across pre-render dots 280..304; `set_vbl_flag` at
(241, 1); flags cleared across the pre-render line from dot 1. Sprite
evaluation ends at dot 131 on lines where eight sprites are found and
at 193 otherwise; the standard world's undefined OAM has Y = 0 in every
slot, so lines 0..7 take the first branch. Coarse X wraps into the next
nametable at tile 33.

**The palette the chip holds is not the palette the world wrote.** The
first stepper picture was wrong in a way no pattern address could
produce, and the chip's own pixel index (`pixel_color0..3`) beside
`pal_d` showed the pattern stream was right and the colours were not.
Read back paced through $2007, the RAM holds `16 2a 12 0f 28 14 02 0f
26 1a 31 0f 30 27 06 20` at $3F00..$3F0F against the sixteen values
the world wrote (`0f 16 2a 12 ...`): every entry one place early, the
backdrop `0x16` rather than `0x0f`, and $3F0F never written, holding
its power-on `0x20`. The write path was then measured on its own
(`examples/p3-write-probe.rs`: a $2006 pair, four known bytes, the row
read back, for every combination of idle after the pair and idle
between the writes):

| idle after the pair | idle between writes | $3F11..$3F14 held (wrote 21 11 01 2a) |
|---|---|---|
| 0, 24, 48, 96, 192 | 0 (back to back) | `11 01 2a 3e` |
| 0, 24, 48, 96, 192 | 24 (an access width) | `31 13 13 3e` |

The idle after the pair changes nothing. Back to back, the first value
is lost and the rest land one entry early, the last of them also
landing once more, ORed, one entry further. Paced, every entry lands
as `data OR (v & 0xff)`, the byte ORed with the low byte of the VRAM
address (`21|11 = 31`, `11|12 = 13`, `01|13 = 13`, `2a|14 = 3e`). One
mechanism gives both rows: **the $2007 write latches its data in the
access's release slot, the same edge at which the harness releases the
CPU data bus.** The chip then samples either the next back-to-back
access's data (one entry early; the first value never sampled) or a
floating bus holding the charge of the value just driven OR the
address phase's charge on the multiplexed AD pins. A real 6502 holds
write data past that point; the harness, which mirrors the reference's
`cpucmd.js` statement for statement, does not, and the reference
script does not either, which is why the P1 node golden agrees with
it. Separately, the die does delay the $2006 low write into v (the
node `delayed_write_2006_low`), which is why the standard world's
first $2007 write also produced a stray CHR-bus write the harness
captured as `$0000 <- 00`. These are findings about the harness's
bus realism and P1's world, not about the stepper, which renders from
the palette as measured; section "Carried to P1" says what they
change.

**The pixel pipeline** (`examples/p3-pal-probe.rs`, per half-step):
`pal_d` precharges to zero through pclk0 and carries the colour through
pclk1 (P1's finding, re-seen); `pal_ptr0..4`, the address the palette
RAM sees, is `pixel_color` one dot later with a zero pattern folded to
index 0 whatever the attribute bits say; `pal_d` is the RAM at
`pal_ptr`; `pixel_color` at hpos h is pixel h - 2 of the line, so pixel
x is on `pal_d` at hpos x + 3, and `pixel_color` is 0 at hpos 0 and 1
on every row probed. With P1's capture convention (hpos h to dot h + 1)
the golden holds pixel x at dot x + 4; the stepper emits pixel x at the
contract's dot x + 1 and the gate compares at +3.

**The fit.** Offset 3, 0 mismatches of 61,440, the minimum by a factor
the neighbours make obvious.

## Step 2: sprites

The datapath gained evaluation, the sprite fetch and the priority mux,
and is held to a second dot golden: the sprite world
(`v2c02_dots::sprite_world`, the standard world plus four sprite
palettes and 64 sprites, sprites on, no left-edge clipping), captured
off rung 0 by `examples/p3-sprites-golden.rs` with the palette RAM and
OAM read back out of the chip beside the dots, and `spr0_hit`'s and
`spr_overflow`'s first rises recorded. The sprites exercise the four
palettes, both flips and their combination, a sprite behind the
background, nine sprites on one line (the eighth is the last drawn),
the right and bottom edges, two overlapping sprites, and P2's sprite 0.

- **Every visible dot agrees with rung 0 with sprites on, 61,440 per
  frame**, on the first run of the datapath against the golden.
- **Sprite 0's hit lands where the chip's does.** `spr0_hit` rose at
  (92, 185) on rung 0; the stepper's first opaque-over-opaque pixel is
  (92, 183), the measured two-dot offset between a pixel's position and
  its arrival at the mux. P2's (91, 182) was a different world with a
  solid background, and does not transfer.
- **The overflow flag.** `spr_overflow` rose at (120, 143), the
  nine-sprite line; the stepper overflows there. The dot is recorded
  for the step that models the evaluation scan dot by dot; the flag is
  held as a fact here.
- `MUTATE=1` drops the sprite pattern fetches from the table and the
  gate goes red.

Evaluation is authored as one step at the sprite window (the chip
spreads it over dots 65..256, measured), the fetch takes the table's
SPR_PT positions slot by slot, and the eight units compose in slot
order with the behind bit honoured against an opaque background.
Sprites of 8x16 are not modelled and asserted off; left-edge clipping
is not modelled and the world does not exercise it.

## Carried to P1, recorded here and not changed

- The P1 report describes "the 16-entry palette" as if the chip held
  what was written. It holds the shifted palette above, for the reason
  the write probe measured. The P1 dot golden and the DAC gate stand as
  measurements of what the chip did (the DAC gate compared level legs
  against the colour on the bus, whichever it was); the description is
  what is wrong. The honest fix is in the harness's bus realism: hold
  the CPU data bus through the release edge the way a 6502 does, in the
  Rust harness AND in the reference's generator script together, then
  regenerate the P1 goldens. Until then any world that loads the
  palette should read it back, as P3's do. A director's call.
- P1's capture places pixel x at dot x + 4 while the contract's active
  window starts at dot 1. Where the picture sits relative to sync is a
  geometry question for the console and the real capture (N5, M4);
  recorded, not moved.
- `capture()` labels its frame `FrameParity::Even`. Aligning on
  (261, 340) is the end of an even frame, so the frame captured is the
  odd one. No visible dot changes.

## Next, inside P3

Sprites (evaluation and fetch in the datapath, held to a dot golden
from the P2 sprite world), the register file and scroll, then the
blargg suites with a CPU attached, then the real capture (handoff
section 5).
