# P3 plan: the ladder, measured before it is built

The console sketch (nes-bus, `docs/nes-end-to-end-v0_2.md`) states
P3's gate in frame time and its order: measure a per-dot stepper
first, and design the bit-sliced datapath now but build it only on a
recorded shortfall. This document is that design, written before the
stepper; `docs/p3-report.md` will carry what was measured.

## What P3 is

An authored fast PPU held to rung 0, the `v6502-micro` pattern: the
sequencer TABLE measured out of the switch-level chip, the DATAPATH
authored from the proven model and labelled as such, the whole held to
rung 0's dot golden. Real time is a P3 property. The gate: **one frame
rendered in at most one frame period**, 714,736 master half-steps at
12 x f_sc with f_sc = 315/88 MHz (ntsc-grid's subcarrier ratio), which
is 16.639 ms; measured over a stated number of frames, mean and worst,
with the margin recorded.

## Step 1: the per-dot table-driven stepper

### The table, measured

One `u16` per dot of a frame (262 lines x 341 dots), the events the
chip performs at that position with rendering enabled, read off rung 0
in the standard world by `crates/v2c02-fast/build.rs` and written to
`OUT_DIR` (NC-SA-derived, never committed, exactly as `v6502-micro`'s
table). The bits, each from a named node or the CHR bus:

| bit | event | measured from |
|---|---|---|
| FETCH_NT, FETCH_AT, FETCH_PT_LO, FETCH_PT_HI | a VRAM address latched while ALE is high, classified by region: below $2000 is a pattern fetch with bit 3 the plane; $2xxx with bits 6..9 all set is an attribute fetch; other $2xxx is a nametable fetch | `PpuPins` ale/ad/a_hi |
| SPR_GARBAGE, SPR_PT_LO, SPR_PT_HI | the same fetches inside the sprite window, which starts where `clear_spr_ptr` pulses at dot 257 and runs eight sprites of eight dots | as above, plus `clear_spr_ptr` |
| INC_X | `load_vramaddr_v_hscroll_next` | node |
| INC_Y | `load_vramaddr_v_vscroll_next` | node |
| COPY_X | `copy_vramaddr_hscroll` | node |
| COPY_Y | `copy_vramaddr_vscroll` | node |
| SET_VBL | `set_vbl_flag` | node |
| CLR_FLAGS | `vbl_clear_flags` | node |

A node event is recorded at a dot if the node is high on any of the
dot's eight half-steps, so a one-half-step pulse is not missed.

What the table deliberately does NOT carry: `fine_y_eq_7_and_rendering`,
`vramaddr_v_hpos_eq_31_and_rendering` and
`vramaddr_v_vpos_29_to_30_transition_and_rendering`. These are
conditions on the v register's value and sit at fixed dots in the
standard world only because scroll is zero. They belong to the
datapath's increment logic, authored, and their measured positions are
a check on that logic, not an input to it.

### Measured positions (2026-09-03, `examples/p3-fetch-probe.rs`)

Recorded before any datapath line was written, from the standard world:

- Background fetches on a visible line: nametable at dot 8k+1,
  attribute at 8k+3, pattern low at 8k+5, pattern high at 8k+7, the
  read landing on the following dot; the two prefetch tiles at 321..335;
  two dummy nametable fetches at 337 and 339. Coarse X wraps into the
  next nametable at tile 33 (dot 249's fetch reads $2401).
- Sprite window 257..320: per sprite two garbage nametable fetches then
  pattern low and high.
- INC_X at dots 8, 16, ..., 256, 328, 336 (34 per line). INC_Y at 256.
  COPY_X at 257 on every rendered line. COPY_Y across pre-render dots
  280..304.
- `set_vbl_flag` at (241, 1). `vbl_clear_flags` high across the
  pre-render line from dot 1. `in_vblank` from (240, 1) through the
  pre-render line.
- Sprite evaluation: on lines where eight sprites are found the scan
  ends at dot 131 (`end_of_oam_or_sec_oam_overflow`); otherwise at 193,
  the end of OAM. The standard world's OAM holds Y = 0 in every slot,
  so lines 0..7 take the first branch. Nothing in step 1 depends on
  this; it is the first measurement step 2 is written from.

### The datapath, authored

Fifteen-bit v and t, fine x; nametable, attribute and two pattern
latches; two 16-bit pattern shifters and the attribute selection; the
32-byte palette RAM; the backdrop. Increment logic per the proven
model (coarse X wraps at 31 toggling the horizontal nametable bit;
fine Y wraps at 7 into coarse Y, which wraps at 29 toggling the
vertical bit, or at 31 without). The delay between the fetch schedule
and the pixel appearing on `pal_d` is **fitted against the dot golden
once and pinned**, the P1 DAC-delay pattern, not typed from a diagram.

### Gates

1. **Dot golden.** `Fast::frame()` equals `goldens/p1-dots.bin` on the
   visible 256 x 240 dots, every dot. The golden's hblank dots 257..340
   carry the pipeline's continuing output and are outside step 1's
   scope, stated here rather than masked.
2. **Frame time.** A stated number of frames timed in a release build;
   mean and worst frame at or under the frame period; the margin
   recorded in the report.
3. **`MUTATE=1`** drops INC_X from the table: the picture cannot be
   right, and the dot comparison must go red.

### After step 1, still inside P3

Sprites (evaluation and fetch in the datapath, held to a dot golden
from the P2 sprite world), then the register file and scroll, then the
blargg suites end to end with a CPU attached (handoff section 5), then
the real capture.

## Step 2: the bit-sliced datapath, budgeted, not built

Authored expectation, to be replaced by step 1's number: a per-dot
stepper does tens of operations per dot, so 89,342 dots land in the
low milliseconds against a 16.639 ms period, a margin of several
times. If the recorded frame time is instead over the period, the first
lever is batching a tile (eight dots) per step, since the fetch
schedule and shifters are tile-periodic; `halfphi::slice`'s lane
encoding parallelises MACHINES, not one frame, so it is a lever for
many consoles, not for one console's real time, and is recorded here as
such. Neither is built until a measurement says so.
