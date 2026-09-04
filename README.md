# 2c02

A switch-level Ricoh 2C02 (the NES PPU), the way the family does
everything: the netlist in, the behaviour out. Companion to
[tinymachines/6502](https://github.com/tinymachines/6502) (the CPU at
the switches) and
[tinymachines/ntsc-crt](https://github.com/tinymachines/ntsc-crt) (the
signal path this chip will feed). The design is
`docs/ppu-handoff-v0_1.md`, ratified 2026-09-02; the milestone log
starts at `docs/p0-report.md`.

## Status

P3 step 1 (the ladder's per-dot stepper) is closed
(`docs/p3-plan.md`, `docs/p3-report.md`): `v2c02-fast` renders the
standard world from a sequencer table measured out of rung 0 at build
time (one event word per dot: the fetches classified by the address
the chip latched, the increments, the copies, the flag events) and a
palette RAM read back out of the chip, with the datapath authored and
labelled. It agrees with rung 0's dot golden on **every visible dot,
62,160 per frame, no exemption**, and renders a frame in **1.004 ms
mean against the 16.639 ms period, 16.6x inside, 996 frames per
second** (worst 1.781 ms), so the plan's bit-sliced datapath is not
built. Getting there produced a finding about P1's world: the 2C02
applies the $2006 low write to v with a delay, the world's first
back-to-back $2007 write went to the stale address, and the palette
the chip holds is every entry one place early with `0x16` as the
backdrop. `MUTATE=1` drops the coarse X increment and goes red.

P2 (the PPU's contested corners) is closed (`docs/p2-report.md`): one
schedule drives sprite-0, the VBL read race and OAM corruption along a
single trajectory the reference replays blindly. Sprite 0 hits at vpos
91, hpos 182 (its authored x plus the two-dot delay), and the two
sprite windows replay **bit-exact node for node, 600 states, no
exemption**. Getting there produced the family's first proven engine
divergence and its fix: the `spr_d` OAM data lines are the eight nodes
the reference special-cases on both-rail groups, and halfphi 0.1.5's
**rail-conflict hold** (an area-weighted charge vote, the reference's
own rule) is what makes the sprite x reach its counter. The VBL race's
three alignments return bit 7 [0, 0, 1] (miss, suppress, consume), a
miss window about a dot and a half wide, cross-checked against the
reference's own sampled D7; OAM shows no corruption under either
documented trigger, byte 2 reading back masked to 0xE3. The race and
corrupt windows are verified behaviourally, for reasons the report
states: a genuine metastability boundary and undefined attribute-bit
DRAM cells are each the wrong thing for a node golden to judge.
`MUTATE=1` goes red on the sprite path.

P0 (the netlist loads and settles) is closed: the 2C02 is the fourth
chip through [halfphi](https://github.com/tinymachines/halfphi)'s
identical calls, 16,758 transistors and 8,770 nodes agreed by both
real parsers, power-on converging unaided, and the reference
simulator's own trace replaying **bit-exact on 10,897 of 10,906 nodes
across 601 states**, the other nine being reset-less sprite latches
whose power-on state silicon itself leaves undefined (the exemption is
closed and any tenth node fails). One finding: 38 supply-gated
transistors that conduct permanently in silicon, absent from every
chip halfphi had met, set conducting by `Ppu::power_on` and proven
load-bearing by mutation.

P1 (the world, first light, the DAC) is closed
(`docs/p1-report.md`): a harness restating the reference's own CPU and
CHR bus machinery, a second node golden that replays the register
program and 3,000 rendering half-steps **through the harness** (3,624
states; the P0 exemption family, 27 nodes here, all flush once
rendering moves real sprite data, and the trailing 1,642 states are
bit-exact on all 10,906 nodes with no mask), first light off the
palette bus into ntsc-crt's DotFrame, and the video DAC held to the
transcribed level table **sample for sample: zero mismatches over
7,680 active samples**, subcarrier phase and pipeline delay fitted
once and pinned. Rendering-on throughput measured at ~32,900
half-steps/s (a frame in about 22 s).

N0 of the console sketch is adopted:
[nes-bus](https://github.com/tinymachines/nes-bus) is the contracts'
home. `DotFrame` is consumed from there (ntsc-crt re-exports it), and
the harness's CHR bus service reads the chip through the contract's
`PpuPins` frame, so the P1 golden now covers the pin frame too:
`MUTATE=rd` flips /RD's polarity in that frame and the replay must go
red.

| Crate | Role |
|---|---|
| `v2c02-netlist` | The die data parsed by halfphi at build time and embedded; builds data-free with a loud refusal when the extern is not fetched. |
| `v2c02-sim` | Power-on and the reference's reset recipe, half-stepping, the node dump the golden comparison rides on, and the harness: the 24-edge CPU register protocol and the CHR/VRAM bus, mirrored from the reference. |
| `v2c02-dots` | From switches to dots: the standard P1 world, frame capture off the palette output bus into ntsc-crt's DotFrame, and the sample-exact DAC comparison. |

## Commands

```bash
bash tools/fetch-netlist.sh          # Quietust's Visual 2C02, eight files,
                                     # sha256-pinned (never committed)
cargo test --workspace --release     # counts, convergence, the three
                                     # goldens, the DAC gate; tests SKIP by
                                     # name without the extern or a golden;
                                     # REQUIRE_NETLIST=1 / REQUIRE_GOLDEN=1
                                     # / REQUIRE_GOLDEN_P1=1 /
                                     # REQUIRE_GOLDEN_P2=1 insist
MUTATE=1 cargo test --workspace --release   # must go red four ways: the
                                     # supply-gated fix-up off, the CHR bus
                                     # serving a wrong byte, the DAC delay
                                     # off by one, the P2 world's background
                                     # made transparent
MUTATE=rd cargo test -p v2c02-sim --release --test golden_p1
                                     # the fifth red, the N0 contract gate:
                                     # /RD's polarity flipped in the nes-bus
                                     # PpuPins frame the CHR bus reads
REQUIRE_GOLDEN_P2=1 cargo test -p v2c02-sim --release --test p2
                                     # P2: sprite-0, the VBL race, OAM, from
                                     # one schedule; sprite windows node-exact,
                                     # race and corrupt behavioural (see the
                                     # report). MUTATE=1 must go red.
REQUIRE_GOLDEN_P3=1 cargo test --release -p v2c02-fast --test p3
                                     # P3 step 1: the per-dot stepper against
                                     # rung 0's dot golden, every visible dot,
                                     # and against the frame period (release
                                     # only). Building v2c02-fast runs the chip
                                     # for a frame (about a minute) to record
                                     # the table and the palette. MUTATE=1
                                     # drops INC_X and must go red.
cargo run --release -p v2c02-fast --example p3-bench -- 500   # frame time
cargo run --release -p v2c02-fast --example p3-fit    # the golden offset, fitted
cargo run --release -p v2c02-dots --example p3-fetch-probe -- /tmp/frame.csv
cargo run --release -p v2c02-dots --example p3-pixel-probe
cargo run --release -p v2c02-dots --example p3-pal-probe
                                     # the three measurements the stepper's
                                     # datapath was authored from (the
                                     # sequencer, the pixel index stream and
                                     # the palette as held)
cargo run --release -p v2c02-sim --example p2-schedule > tools/golden-trace/p2-schedule.json
                                     # recompute the P2 schedule (the dry run
                                     # on this engine; both sides replay it)
node tools/golden-trace/gen-p2.js    # regenerate the P2 golden (1,712
                                     # windowed states, ~3 h)
node tools/golden-trace/gen.js       # regenerate the P0 trace
                                     # (601 states, about 5 s)
node tools/golden-trace/gen-p1.js    # regenerate the P1 trace (712,100
                                     # pre-roll + 3,624 states, ~40 min)
cargo run --release -p v2c02-sim --example bench        # quiescent throughput
cargo run --release -p v2c02-dots --example p1-bench    # rendering-on throughput
cargo run --release -p v2c02-dots --example first-light # goldens/p1-first-light.ppm
cargo run --release -p v2c02-sim --example p1-diverge-probe
                                     # the measurement the P1 exemption
                                     # is written from
```

## Licensing

The code is MIT. The die data (`extern/visual2c02/`, fetched, never
committed) is Quietust's Visual 2C02, derived from the visual6502
team's CC BY-NC-SA imagery; see `NOTICE.md`. NonCommercial and
ShareAlike propagate to any artifact embedding it.
