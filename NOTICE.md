# Notices

The code in this repository is MIT (see `LICENSE`). What it consumes
is not:

- `extern/visual2c02/` (fetched by `tools/fetch-netlist.sh`, pinned by
  sha256, **never committed**): Quietust's Visual 2C02 die data and
  simulator (segdefs, transdefs, nodenames, wires.js, chipsim.js),
  from `www.qmtpro.com/~nes/chipimages/visual2c02/`. The pages carry
  no explicit licence text (checked 2026-09-02); the netlist derives
  from the visual6502 team's RP2C02 die photography, which is CC
  BY-NC-SA, so this repository treats the data as NC-SA with
  attribution to Quietust and visual6502.org. **NonCommercial and
  ShareAlike propagate to any artifact embedding it**, which includes
  any build of `v2c02-netlist` made with the extern present. Whether
  to confirm terms with Quietust directly is an open courtesy item
  (docs/ppu-handoff-v0_1.md, question 4).
- The golden trace (`tools/golden-trace/golden-2c02.txt`, gitignored)
  is generated locally by running that simulator and is derived data
  under the same terms.
