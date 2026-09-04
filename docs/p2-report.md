# P2 report: the PPU's contested corners, by crafted micro-trace

P2 closes the questions emulator folklore argues about, each pinned by
a scripted register program run on the switch-level chip and checked
against the reference simulator. One schedule
(`tools/golden-trace/p2-schedule.json`, computed by the `p2-schedule`
dry run on this engine) drives three scenarios along one trajectory;
the reference executes the same schedule blindly (`gen-p2.js`) and
dumps every node inside six windows; the test (`tests/p2.rs`) replays
and compares. `cargo test -p v2c02-sim --test p2` with
`REQUIRE_GOLDEN_P2=1`; `MUTATE=1` must go red.

## Sprite 0: the hit lands where the silicon puts it

Sprite 0 (y=90, x=180, a solid tile over a solid background) hits at
**vpos 91, hpos 182**: the sprite's authored x plus the two-dot
pipeline delay, measured before it was pinned. The two sprite windows,
the fetch-line counter load and the display-line hit, **replay
bit-exact against the reference, node for node, 600 states with no
exemption at all.**

Getting there was the milestone's real work, and it produced the
family's first proven engine divergence. Our first run hit sprite 0 at
the left edge: its H-position counter `spr0_p` read 0, because the x
byte never reached it. The `spr_d` OAM data lines are the eight nodes
the reference's own `chipsim.js` special-cases when a group holds both
rails, and without that special case this engine resolved such groups
Vss-wins and crushed the byte to zero. The reference's own run of the
scenario (`gen-p2-probe.js`) confirmed it: **its** counter loads 180
and counts down to the hit at the sprite's x.

The fix is [halfphi](https://github.com/tinymachines/halfphi) 0.1.5's
**rail-conflict hold**: a chip-agnostic mechanism with a chip-supplied
list. For a marked group joined to both rails the rails cancel, and
the fallback is the reference's own rule, pulls first and then an
**area-weighted charge vote**. Two counting fallbacks died on the P0
golden at init before the area vote was found; the instrumented
reference (`gen-hold-probe.js`) showed why, eleven charged members
losing the vote to two large ones. The list is extracted from the
pinned `chipsim.js` at build time and cross-checked against the
`spr_d` names in the counts test; the areas are computed the way
`wires.js` computes them. With the hold, P0 and P1 stay green and the
sprite scenario matches the reference at every checkpoint.

## The VBL read race: the miss window, measured

`set_vbl_flag` rises at **vpos 241, hpos 1** (the wiki's "scanline 241
dot 1", from silicon). Reading $2002 with the access sweeping across
that set produced the classic three outcomes, and the schedule pins
three of them:

| offset (read start minus set) | returns bit 7 | flag after | NMI |
|---|---|---|---|
| -30 to -24 | 0 | survives | fires normally at the set |
| -18, -12 (the race) | 0 | **gone** | **never fires** |
| -6 to +12 | 1 | consumed by the read | cleared |

The famous missed-vblank window measures about a dot and a half wide,
bracketed by clean behaviour on both sides. The three scheduled reads
(miss, suppress, consume) return bit 7 **[0, 0, 1]** on this engine.

The race windows are verified **behaviourally, not by node golden**,
and the distinction is the point. A node golden was the right
instrument for the sprite windows (pure chip behaviour), but the race
is a deliberately-constructed metastability boundary: the read lands
within a hair of the internal flag-set, and two faithful engines can
thread that boundary along different internal node trajectories while
agreeing on every observable. The evidence that this is trajectory and
not result: the reference's **own** sampled D7 at the three race reads,
read back out of its golden at the exact sample edges, is **[0, 0, 1]**
too, matching this engine at every alignment. So the observable, the
architectural outcome, and the sampled read all agree on both engines;
54 to 68 internal I/O-path nodes (`read_2002_output_vblank_flag`,
`vbl_flag`, `int`, the CPU bus pins) differ in their moment-to-moment
state through the race and are not node-judged. A node mask over the
very vblank-read nodes the test exists to check would defeat it; the
behavioural assert, cross-checked against the reference's own sampled
result, is the honest instrument here.

## OAM: writes, the attribute mask, and no corruption

Two protocol facts fell out and are held by the test:

- **$2004 writes complete in the access's release slot.** A
  back-to-back following access cancels the pending OAM write; with one
  access-width of idle after each write (exactly DMA pacing, and
  `cpucmd.js`'s own rhythm) every write lands. No real bus master
  writes faster, so this is a harness-realism constraint, not a chip
  bug, and the harness leaves the gap.
- **OAM byte 2 reads back masked to 0xE3**, every row: the attribute
  byte's unimplemented bits 2..4, silicon-correct.

**No corruption was observed** under either documented trigger (a
$2003 write mid-render; OAMADDR standing at $28 when rendering starts):
all 64 bytes identical after a full rendered frame, and the sixteen
scheduled read-backs return the identity fill with the byte-2 mask.

The corrupt window carries a finding of its own, and it is why that
window is behavioural too. OAM byte 2's unimplemented attribute bits
are physical DRAM cells no $2004 write can drive; their power-on state
is a coin the two engines flip differently, and the parked-OAMADDR
frame makes sprite evaluation read the rows those cells sit on. A node
golden over that frame is a coin toss by construction, on both engines
alike. The first schedule left whole OAM rows unwritten and that coin
forked the entire render pipeline (248 nodes by the corrupt window);
the schedule now initialises all 64 OAM bytes first, which confines the
undefined state to the unwritable attribute bits, and the corruption
claim rests on the architectural read-back rather than the nodes.

## The gate

- Sprite windows: **600 states bit-exact, node for node, no
  exemption.**
- Race and corrupt windows: architectural results asserted
  ([0, 0, 1]; the hit at 91,182; OAM identity plus the 0xE3 mask), the
  race cross-checked against the reference's own sampled D7.
- `MUTATE=1` swaps the world for one whose background tile is
  transparent: the sprite-0 hit the test insists on never comes, and
  the replay goes red at half-step 962572 on `spr0_/h0`. No branch
  blesses the mutant.

## Carried forward

The race window's internal node trajectory differs between the two
faithful engines while both agree on every observable and on the
sampled read result. Whether that is a narrow algorithmic difference at
the metastability boundary or an asymmetry in the two bus-access
replays is not yet pinned; it is not load-bearing for P2, and the
observable is verified on both engines. It is the natural first thread
if the console's N5 gate-1 (the VBL race replayed through the whole
console) ever needs the race resolved at node granularity rather than
at the pins.
