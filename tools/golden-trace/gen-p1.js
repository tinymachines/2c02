#!/usr/bin/env node
// The P1 oracle: the reference engine driven through the SAME world the
// Rust harness builds, dumped over the interesting window.
//
// Script, counted in half-steps after init (matching Harness.half_steps):
//   0 .. PRE_ROLL          free run: the measured warm-up (after_reset_reg
//                          clears at half-step 712,010; PRE_ROLL adds margin)
//   then 24 CPU writes     the reference's 24-edge protocol (cpucmd.js):
//                          ctrl/mask zero, palette loaded via $2006/$2007,
//                          address parked, scroll zeroed, background on
//   then RENDER_STEPS      rendering runs against the shared VRAM function
//
// Nodes are dumped from the first write onward. The CHR bus is served
// exactly as macros.js handleChrBus does, with mRead replaced by the
// shared vram() so both engines see the same world.
//
// Slow by nature (the reference runs at roughly 125 half-steps/s here):
// run it in the background and go build something.

const fs = require('fs');
const path = require('path');
const vm = require('vm');

const REF = path.resolve(__dirname, '../../extern/visual2c02');
const OUT = path.resolve(__dirname, 'golden-2c02-p1.txt');

const PRE_ROLL = 712_100;
const RENDER_STEPS = 3_000;

const PALETTE = [0x0f, 0x16, 0x2a, 0x12, 0x0f, 0x28, 0x14, 0x02,
                 0x0f, 0x26, 0x1a, 0x31, 0x0f, 0x30, 0x27, 0x06];
// (rw, reg, val): the register program, mirrored in the Rust tests.
// The $2002 read first: it resets the $2005/$2006 write toggle, whose
// post-reset state is undefined, and without it the palette writes land
// at a garbage address (measured before this line existed).
const WRITES = [
  [1, 2, 0x00],
  [0, 0, 0x00], [0, 1, 0x00],
  [0, 6, 0x3f], [0, 6, 0x00],
  ...PALETTE.map(v => [0, 7, v]),
  [0, 6, 0x20], [0, 6, 0x00],
  [0, 5, 0x00], [0, 5, 0x00],
  [0, 1, 0x0a],
];

const sandbox = {
  console,
  window: {},
  document: { getElementById: () => null, createElement: () => ({ appendChild() {}, childNodes: [] }), createTextNode: () => ({}) },
  navigator: { appVersion: '', appName: 'node' },
  location: { search: '' },
  setTimeout,
};
sandbox.global = sandbox;
vm.createContext(sandbox);
const load = (f) => vm.runInContext(fs.readFileSync(path.join(REF, f), 'utf8'), sandbox, { filename: f });
load('segdefs.js');
load('transdefs.js');
load('nodenames.js');
load('wires.js');
load('chipsim.js');

vm.runInContext(
  `
  setupNodes();
  setupTransistors();
  for (var nn in nodes) { nodes[nn].state = false; nodes[nn].float = true; }
  nodes[ngnd].state = false; nodes[ngnd].float = false;
  nodes[npwr].state = true;  nodes[npwr].float = false;
  for (var tn in transistors) transistors[tn].on = (transistors[tn].gate == npwr);
  setLow('res'); setLow('clk0'); setHigh('io_ce'); setHigh('int');
  recalcNodeList(allNodes());
  for (var i = 0; i < 4; i++) { setHigh('clk0'); setLow('clk0'); }
  setHigh('res');

  vram = function (a) { return ((a >> 4) ^ a) & 0xff; };
  chrAle = isNodeHigh(nodenames['ale']);
  chrRd = isNodeHigh(nodenames['rd']);
  chrWr = isNodeHigh(nodenames['wr']);
  chrAddr = 0;
  writeBits = function (name, n, x) {
    var recalcs = [];
    for (var i = 0; i < n; i++) {
      var nn = nodenames[name + i];
      if ((x % 2) == 0) { nodes[nn].pulldown = true; nodes[nn].pullup = false; }
      else { nodes[nn].pulldown = false; nodes[nn].pullup = true; }
      recalcs.push(nn); x >>= 1;
    }
    recalcNodeList(recalcs);
  };
  floatBits = function (name, n) {
    var recalcs = [];
    for (var i = 0; i < n; i++) {
      var nn = nodenames[name + i];
      nodes[nn].pulldown = false; nodes[nn].pullup = false;
      recalcs.push(nn);
    }
    recalcNodeList(recalcs);
  };
  readBits = function (name, n) {
    var v = 0;
    for (var i = 0; i < n; i++) if (isNodeHigh(nodenames[name + i])) v |= (1 << i);
    return v;
  };
  handleChr = function () {
    var ale = isNodeHigh(nodenames['ale']);
    var rd = isNodeHigh(nodenames['rd']);
    var wr = isNodeHigh(nodenames['wr']);
    if (!chrAle && ale) chrAddr = readBits('ab', 14);
    if (chrRd && !rd) writeBits('db', 8, vram(chrAddr & 0x3fff));
    if (!chrRd && rd) floatBits('db', 8);
    chrAle = ale; chrRd = rd; chrWr = wr;
  };
  halfStep = function () {
    if (isNodeHigh(nodenames['clk0'])) setLow('clk0'); else setHigh('clk0');
    handleChr();
  };
  maxNode = nodes.length;
  dump = function () {
    var s = '';
    for (var i = 0; i < maxNode; i++) s += (nodes[i] !== undefined && isNodeHigh(i)) ? '1' : '0';
    return s;
  };
`,
  sandbox
);
const run = (expr) => vm.runInContext(expr, sandbox);

const lines = [`2c02 p1 golden: nodes ${run('maxNode')} preroll ${PRE_ROLL} writes ${WRITES.length} render ${RENDER_STEPS}`];
const t0 = Date.now();
for (let i = 0; i < PRE_ROLL; i++) {
  run('halfStep()');
  if (i % 10000 === 9999) {
    const rate = (i + 1) / ((Date.now() - t0) / 1000);
    process.stderr.write(`\rpre-roll ${i + 1}/${PRE_ROLL} at ${rate.toFixed(0)}/s, eta ${((PRE_ROLL - i) / rate / 60).toFixed(0)} min`);
  }
}
process.stderr.write('\npre-roll done, writing registers\n');
for (const [rw, reg, val] of WRITES) {
  for (let counter = 24; counter >= 1; counter--) {
    run(`
      (function (counter, rw, reg, val) {
        if (counter == 24) {
          writeBits('io_ab', 3, reg);
          if (rw) floatBits('io_db', 8); else writeBits('io_db', 8, val);
          if (rw) setHigh('io_rw'); else setLow('io_rw');
        }
        if (counter == 16) setLow('io_ce');
        if (counter == 1) setHigh('io_ce');
      })(${counter}, ${rw}, ${reg}, ${val});
      halfStep();
    `);
    lines.push(run('dump()'));
  }
  run("floatBits('io_db', 8)");
}
for (let i = 0; i < RENDER_STEPS; i++) {
  run('halfStep()');
  lines.push(run('dump()'));
  if (i % 200 === 199) process.stderr.write(`\rrender ${i + 1}/${RENDER_STEPS}`);
}
process.stderr.write('\n');
fs.writeFileSync(OUT, lines.join('\n') + '\n');
console.log(`wrote ${OUT} (${lines.length - 1} states)`);
