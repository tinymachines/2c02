# P0 report: the netlist loads and settles

Run stamp: 2026-09-02, first commit of this repository, rustc 1.97.1,
halfphi v0.1.2 (the published crate, untouched), `cargo test
--workspace` 3 tests green, MUTATE=1 red where it must be. The handoff
sketch is docs/ppu-handoff-v0_1.md, ratified by the director
2026-09-02 with the repository named tinymachines/2c02.

## What closed

- **The 2C02 is the fourth chip through halfphi's identical calls**,
  with zero parser changes: `parse(ChipSource { rails: gnd/pwr })`,
  `Netlist::decode`, `Engine::new`. The third rail spelling in the
  family (6502 vss/vcc, 6800 gnd/vcc, 2C02 gnd/pwr) is why rails were
  a parameter.
- **Counts, agreed by both real parsers**: 16,758 transistors, 8,770
  nodes (ids to 10,905), 858 names. The sketch's first number, 16,871,
  was a regex counting the 114 transistors Quietust left inside a
  block comment in transdefs.js; a second comment-aware regex said
  16,757, fumbling one entry. halfphi and the reference JS engine
  itself agree on 16,758, and the counts test records the saga:
  instruments lie before the code does.
- **Power-on converges with no chip-specific help** (zero
  nonconvergent settles), the same claim the 6800 and Z80 make.
- **One piece of real chip knowledge was needed**, and it is the
  finding of the milestone: the 2C02 has **38 supply-gated
  transistors** (the 6502 has none). In silicon they conduct
  permanently; the reference's init turns them on; halfphi's power-on
  starts every transistor off, and since rails are never recalculated
  nothing would ever switch them. `Ppu::power_on` sets exactly those
  38 conducting once, documented in place, and the MUTATE=1 proof
  switches them back off: the replay then diverges at step 0
  (an earlier mutation floated the io_ce pin instead, and the node
  simply held its charge; a mutation the subject survives by design
  proves nothing, so it was replaced).
- **The reference trace replays.** tools/golden-trace/gen.js runs
  Quietust's own engine headlessly (wires.js + chipsim.js only, the
  init and stepping implemented statement for statement from
  macros.js, both buses undriven on both sides), 601 states over
  10,906 node slots. The Rust engine matches **bit for bit on 10,897
  of 10,906 nodes across all 601 states**.

## The nine that differ, and why the list is closed

Nine sprite-path input latches are dynamic storage with no reset
connection: their power-on state is genuinely undefined, and the two
engines flip that coin deterministically but differently. Measured:
six (the sprite data-in latches and their derived terms, two of them
unnamed) flush to agreement by half-step 14, when real values first
move through them; three (the x-flip input latch and its followers)
are never written in a memory-less free-run and hold their coin for
the whole trace. The golden test exempts exactly these, by name where
names exist, with the early/late split at the measured flush step, and
any tenth node fails the run: a masked comparison is only as honest as
its mask is small, and the 6502's rail-write bug hid behind exactly
this shape of blindness.

## Throughput, measured (the sketch's estimate replaced)

Rung 0 runs at **50,457 half-steps/s** on the family's Ryzen 5 5600X:
faster than the 6502's 29,600 despite 4.8x the transistors, because a
reset chip with rendering off is mostly quiescent and a settle walks
only the active cone. A full frame at this state is about 14 seconds,
not the sketch's authored "minutes". Rendering on will be slower and
P1 will measure it rather than guess.

## Carried forward

- P1: the bus harness (VRAM, palette, OAM, registers), free-running
  render, the DotFrame extraction, first light through ntsc-crt, and
  the DAC tap (`vid_luma*`, `vid_burst_*`, `vid_emph` are all named
  nodes) held against data/nes-levels.toml with tolerances stated
  first.
- The x-flip latch trio will leave the exemption list the moment P1
  writes real sprite data through it; the exemption shrinking is
  itself a P1 assertion worth making.
- Open from the sketch: whether DotFrame moves to a shared contract
  crate, and the Quietust licence courtesy note.

## Superseded 2026-09-04: there was no undefined latch

The nine-node exemption above was the engine, not the silicon. halfphi
0.1.6 resolves a group with no rail and no pull by the reference's own
area vote (until then any one charged member made it high; the 2C02's
`getNodeValue` weighs the members' areas), and under it the P0 golden
replays bit-exact on all 10,906 nodes across all 601 states with no
mask, the x-flip trio included. P1's wider family closed the same way
(`docs/p1-report.md`, its section of this date); the write-path
divergence that led there is in `docs/p3-report.md`. The test compares
every node; MUTATE=1 still goes red at step 0.
