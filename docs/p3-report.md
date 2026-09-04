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
its power-on `0x20`. The cause is on the die: the 2C02 applies the
$2006 low write to v with a delay (the netlist names the node
`delayed_write_2006_low`), and the world issues its first $2007 write
back to back, so that write went to the stale address; the harness
captured it as a VRAM write, `$0000 <- 00`. This is a finding about
P1's world, not about the stepper, and section "Carried to P1" says
what it changes.

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

## Carried to P1, recorded here and not changed

- The P1 report describes "the 16-entry palette" as if the chip held
  what was written. It holds the shifted palette above. The P1 dot
  golden and the DAC gate stand as measurements of what the chip did
  (the DAC gate compared level legs against the colour on the bus,
  whichever it was); the description is what is wrong. The world can
  be paced so the first $2007 write lands, which regenerates the P1
  golden and picture, or the report can say what the palette is. A
  director's call.
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
