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

| Crate | Role |
|---|---|
| `v2c02-netlist` | The die data parsed by halfphi at build time and embedded; builds data-free with a loud refusal when the extern is not fetched. |
| `v2c02-sim` | Power-on and the reference's reset recipe, half-stepping, the node dump the golden comparison rides on. |

## Commands

```bash
bash tools/fetch-netlist.sh          # Quietust's Visual 2C02, five files,
                                     # sha256-pinned (never committed)
cargo test --workspace               # counts, convergence, the golden
                                     # replay; tests SKIP by name without
                                     # the extern or the golden;
                                     # REQUIRE_NETLIST=1 / REQUIRE_GOLDEN=1
MUTATE=1 cargo test --workspace      # must go red: the supply-gated
                                     # fix-up switched off
node tools/golden-trace/gen.js       # regenerate the reference trace
                                     # (601 states, about 5 s)
cargo run --release -p v2c02-sim --example bench   # rung-0 throughput
```

## Licensing

The code is MIT. The die data (`extern/visual2c02/`, fetched, never
committed) is Quietust's Visual 2C02, derived from the visual6502
team's CC BY-NC-SA imagery; see `NOTICE.md`. NonCommercial and
ShareAlike propagate to any artifact embedding it.
