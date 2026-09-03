# P2 report, DRAFT (measurements banked; the sprite-0 question still open)

NOT A CLOSED MILESTONE. This draft holds the measurements already made
so they are not retyped from memory; it becomes docs/p2-report.md when
the sprite-0 divergence question is settled and the goldens exist.
Nothing here is golden-confirmed yet unless it says so.

## Measured so far (Rust side, the engine P0/P1 proved bit-exact)

- **VBL set position**: `set_vbl_flag` rises at vpos 241, hpos 1, the
  wiki's "scanline 241 dot 1" measured from silicon.
- **Frame lengths**: consecutive sets 714,728 half-steps apart with
  rendering on (the odd-frame skipped dot; ntsc-grid's OddShort
  constant, met in silicon) and 714,736 with rendering off (the full
  frame). Two constants from another repo, confirmed for free.
- **The VBL read race**, eight alignments of the 24-half-step access
  around the set (offset = access start minus set):
  | offset | returns bit7 | flag after | NMI |
  |---|---|---|---|
  | -30 | 0 | survives | fires at set, normally |
  | -24 | 0 | survives | fires |
  | -18, -12 | 0 | GONE | NEVER FIRES |
  | -6 .. +12 | 1 | consumed | cleared by the read |
  The famous missed-vblank window measured about a dot and a half wide,
  bracketed by clean behaviour.
- **$2004 writes complete in the release slot**: a back-to-back
  following access cancels the pending OAM write; with one access-width
  of idle after each write (exactly DMA pacing, and cpucmd.js's own
  rhythm) every write lands. No real bus master writes faster, so this
  is a harness-realism constraint, not a chip bug.
- **OAM byte 2 masks to 0xE3** on read-back, every row tried: the
  attribute byte's unimplemented bits 2..=4, silicon-correct.
- **OAM corruption: none observed** under either documented trigger
  (a $2003 write mid-render; OAMADDR standing at $28 when rendering
  starts), all 64 bytes identical after a full rendered frame.
  Interpretation BLOCKED on the sprite-0 question below: the row-copy
  mechanism would run over the same spr_d bus.

## The open question, CLOSED: the rail-conflict hold

The reference's own run of the sprite scenario (gen-p2-probe.js)
answered: its spr0_p loads 180 on the fetch line and counts down to
the hit at the sprite's authored x; ours loaded 0. The cause is the
spr_d OAM data lines, the eight nodes the reference's chipsim
special-cases when a group holds both rails. halfphi 0.1.5 carries the
generic fix, the rail-conflict hold: the rails cancel for marked
groups and the fallback is the reference's own rule, pulls first, then
an AREA-WEIGHTED charge vote (two counting fallbacks died on the P0
golden first; the instrumented reference, gen-hold-probe.js, showed
eleven charged members losing the vote to two large ones). The list is
extracted from the pinned chipsim.js at build time and cross-checked
against the spr_d names; the areas are computed the way wires.js
computes them. With the hold: P0 and P1 replay green, and the sprite
scenario matches the reference at every checkpoint.

## The old open question, kept for the record

Sprite 0 (y=90, x=180, solid tile over solid background, OAM verified
loaded) hits at the display line y+1 but at hpos ~2, and its H-position
counter `spr0_p` reads 0 at every sampled point: the x byte never
reaches the counter, so the sprite renders at the left edge. The spr_d
OAM data lines are the eight nodes the reference's own chipsim
special-cases when a group holds both rails (P0's gnd/pwr finding), and
our engine resolves such groups Vss-wins. Whether the reference loads
spr0_p = 180 in the same scenario is being answered by
tools/golden-trace/gen-p2-probe.js (the reference itself, same script,
same checkpoints). Divergent answers mean a resolution-rule fix
designed the 0.1.3 way; matching answers mean this extraction genuinely
renders sprite x this way and the scenario documents it.
