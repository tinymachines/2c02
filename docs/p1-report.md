# P1 report: the world, first light, and the DAC held to the table

Run stamp: 2026-09-02, rustc 1.97.1, halfphi v0.1.2 (still untouched).
`cargo test --workspace --release` with `REQUIRE_GOLDEN_P1=1`: 5 tests
green (counts, convergence, the P0 golden, the P1 golden through the
harness, the DAC gate). Three independent MUTATE=1 proofs go red: the
supply-gated fix-up switched off (P0), the CHR bus serving every
nametable byte off by one bit (the P1 replay diverges at `_db0`, state
623), and the DAC pipeline delay perturbed by one half-step.

## Re-recorded 2026-09-04

Two things P3 measured changed this world and what its golden says
(`docs/p3-report.md`, the write-path section):

- **The palette lands as written now.** The register program wrote
  its sixteen palette entries back to back, and back to back both
  engines lose the first `$2007` value and land the rest one entry
  early, so the world this report first described held `0x16` as its
  backdrop and every entry one place off; nothing here could see it
  (the DAC gate compares legs against whatever colour is on the bus,
  and first light was eyeballed). The program now follows each palette
  write with an access width of idle, in `standard_program` and in
  `gen-p1.js` alike, and the chip holds `0f 16 2a 12 0f 28 14 02 0f
  26 1a 31 0f 30 27 06`, read back paced. The first `$2007` access
  after the `$2006` pair still puts one bus cycle out on the stale
  address (`$0000 <- 00`, the delayed low write), which a
  pure-function world cannot feel; recorded for the console.
- **No exemption.** halfphi 0.1.6 resolves an undriven group by the
  reference's own area vote (0.1.5 and before made it high on any one
  charged member). Re-recorded under it, the P1 golden replays
  **bit-exact on all 10,906 nodes across all 4,008 states** (26
  accesses of 24 edges, 384 half-steps of idle, 3,000 rendering
  half-steps), and `p1-diverge-probe` reports zero nodes that ever
  diverge. The 27-node family below, read on 2026-09-02 as undefined
  power-on state flushing when real sprite data moved, was the
  engine's charge rule deciding floating sprite-path groups
  differently from the reference; there was no coin. The test
  compares every node from the first state; both mutations still go
  red at the first CHR fetch after rendering starts (state 1,007).
- The dot golden, first light and the DAC gate were regenerated from
  the re-recorded world: the DAC fit lands on the same pinned phase
  and delay, zero mismatches over 7,680 samples; first light's
  backdrop is `0x0f`.

The sections below are the 2026-09-02 report as written, kept as the
record of what was measured then and how it was read.

## What closed

- **The harness is the reference's own machinery, restated.** The
  CPU-side register protocol is cpucmd.js's 24-edge access (address
  and R/W at edge 24, chip enable at 16, release and sample at 1); the
  CHR/VRAM bus is macros.js's handleChrBus (address latched on ALE
  rising, data driven on /RD falling and floated on its rising, writes
  captured on /WR rising), run after each half-step in the reference's
  own order. VRAM is a pure function of address, so the golden
  generator scripts the identical world in JS.
- **The P1 node golden replays through the harness.** 712,100
  half-steps of pre-roll, then a $2002 read, the register program (26
  accesses: ctrl and mask zero, the palette loaded through $2006/$2007,
  the address parked in the nametable, scroll zeroed, background
  rendering on) and 3,000 rendering half-steps: 3,624 dumped states,
  every access edge compared, so the protocol itself is under the
  oracle, not just the chip.
- **The exemption family flushed, x-flip trio included.** P0 exempted
  nine reset-less sprite-path latches and predicted the x-flip trio
  would leave the list when real sprite data moved through it. Measured
  here (`examples/p1-diverge-probe.rs`): the family is 27 nodes wide in
  this longer trace (the trio, all eight `spr_dN_in` latches, their
  range-gated followers, eight unnamed followers), and once rendering
  is on and sprite evaluation writes through the path **every one of
  them converges: the last divergent state is 1,981, and the trailing
  1,642 states are bit-exact across all 10,906 nodes with no mask at
  all.** The test pins the flush state and runs unmasked past it; a
  28th node, or a divergence after the flush, fails the run.
- **First light.** `examples/first-light.rs` captures four rows of the
  standard world off the palette output bus (`pal_d0..5_out`, sampled
  in each dot's pclk1 phase, a measurement made before the capture was
  written) into ntsc-crt's DotFrame and writes
  `goldens/p1-first-light.ppm`: the tile grid, attribute quadrants and
  stripe patterns of the XOR VRAM function, eyeballed correct against
  the register program.
- **The DAC gate: zero mismatches over 7,680 active samples.** One
  half-step is one grid sample (the master half-clock is 12 x f_sc),
  so ntsc-source-nes's own signal function predicts, sample for
  sample, which of the twelve level legs conducts. Two constants were
  fitted once and pinned: the subcarrier phase offset (11, from the
  burst legs, which are exactly wave 8) and the DAC pipeline delay
  behind the pal bus (12 half-steps). The test re-fits on every run and
  holds the fit to the pins, so a drift fails rather than silently
  re-aligning; structure asserts ride along (exactly one leg conducts
  at every half-step, the emphasis attenuator never does with no
  emphasis bits set).

## Measured en route

Facts that were measured before the code that depends on them was
written, in the order they were needed:

- The reset gate releases at half-step **712,010**; the world warms up
  712,100 and then talks.
- **A $2002 read must precede the address writes**: the $2005/$2006
  write toggle's post-reset state is undefined, and without the read
  the palette landed at a garbage address.
- `pal_d0..5_out` carries the pixel's colour during the **pclk1**
  phase of each dot and precharges during pclk0.
- The eleven DAC level legs were calibrated **from a scanline's known
  geography, not from their names**: `vid_sync_l` is the sync tip and
  `vid_sync_h` is the *blanking* level, which is why the $xE/$xF
  blacks assert it, exactly as the transcribed table says those
  columns output the blank voltage.

## Throughput, rendering on (P0's deferred number)

**~32,900 half-steps/s** best-of-three with background rendering
enabled (`examples/p1-bench.rs`), against 50,457 quiescent: a full
262-line frame is 714,736 half-steps, about 22 seconds. The active
cone with rendering on is roughly half the chip's quiet cost again,
and a faster rung is a later milestone's problem, measured here first.

## Carried forward

- P2 per the sketch: the dot stream into ntsc-crt's full encode/decode
  path and a composite frame scored, not eyeballed.
- Sprite rendering (OAM writes through $2003/$2004) is untouched; the
  27-node flush measured here says the sprite path is live and
  agreeing once exercised. (Superseded 2026-09-04: there is no
  family; see the section at the top.)
- Open from P0 still open: DotFrame's home, the Quietust licence
  courtesy note.
