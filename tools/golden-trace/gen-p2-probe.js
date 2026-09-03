#!/usr/bin/env node
// The reference's own answer to the sprite-0 scenario: the P1 warm-up
// and register program, paced $2004 OAM writes (sprite 0 at y=90,
// x=180 over a solid background), rendering on, then the same
// checkpoints the Rust probe prints: OAM read back, and spr0_p with
// spr0_hit through the fetch and display lines. Slow (the reference
// runs at ~125 half-steps/s): run in the background.

const fs = require('fs');
const path = require('path');
const vm = require('vm');

const REF = path.resolve(__dirname, '../../extern/visual2c02');

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
  setLow('res'); setLow('clk0');
  setHigh('io_ce'); setHigh('int');
  recalcNodeList(allNodes());
  for (var i = 0; i < 4; i++) { setHigh('clk0'); setLow('clk0'); }
  setHigh('res');

  // The world: tile 1 solid, every nametable entry tile 1, attributes 0.
  vram = function (a) {
    a &= 0x3fff;
    if (a >= 0x0010 && a <= 0x001f) return 0xff;
    if (a < 0x2000) return 0x00;
    var nt = a & 0x0fff;
    return (nt & 0x03ff) < 0x03c0 ? 0x01 : 0x00;
  };

  // The harness world, statement for statement as gen-p1.js.
  chr_ale = isNodeHigh(nodenames['ale']);
  chr_rd = isNodeHigh(nodenames['rd']);
  chr_wr = isNodeHigh(nodenames['wr']);
  chr_addr = 0;
  readBits = function (name, n) {
    var res = 0;
    for (var i = 0; i < n; i++) res += (isNodeHigh(nodenames[name + i]) ? 1 : 0) << i;
    return res;
  };
  writeBits = function (name, n, x) {
    var recalcs = [];
    for (var i = 0; i < n; i++) {
      var nn = nodenames[name + i];
      var node = nodes[nn];
      if ((x % 2) == 0) { node.pulldown = true; node.pullup = false; }
      else { node.pulldown = false; node.pullup = true; }
      recalcs.push(nn);
      x >>= 1;
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
  handleChrBus = function () {
    var ale = isNodeHigh(nodenames['ale']);
    var rd = isNodeHigh(nodenames['rd']);
    var wr = isNodeHigh(nodenames['wr']);
    if (!chr_ale && ale) chr_addr = readBits('ab', 14);
    if (chr_rd && !rd) writeBits('db', 8, vram(chr_addr & 0x3fff));
    if (!chr_rd && rd) floatBits('db', 8);
    chr_ale = ale; chr_rd = rd; chr_wr = wr;
  };
  halfStep = function () {
    if (isNodeHigh(nodenames['clk0'])) setLow('clk0'); else setHigh('clk0');
    handleChrBus();
  };
  halfSteps = 0;
  wait = function (n) { for (var i = 0; i < n; i++) { halfStep(); halfSteps++; } };

  // cpucmd's 24-edge access, as the Rust harness mirrors it.
  accessEdge = function (rw, reg, val, counter) {
    var sampled = null;
    if (counter == 24) {
      writeBits('io_ab', 3, reg);
      if (rw) floatBits('io_db', 8); else writeBits('io_db', 8, val);
      if (rw) { nodes[nodenames['io_rw']].pulldown = false; nodes[nodenames['io_rw']].pullup = true; }
      else { nodes[nodenames['io_rw']].pulldown = true; nodes[nodenames['io_rw']].pullup = false; }
      recalcNodeList([nodenames['io_rw']]);
    }
    if (counter == 16) { setLow('io_ce'); }
    if (counter == 1) {
      if (rw) sampled = readBits('io_db', 8);
      setHigh('io_ce');
    }
    return sampled;
  };
  cpuAccess = function (rw, reg, val) {
    var sampled = 0;
    for (var counter = 24; counter >= 1; counter--) {
      var d = accessEdge(rw, reg, val, counter);
      if (d !== null) sampled = d;
      halfStep(); halfSteps++;
    }
    floatBits('io_db', 8);
    return sampled;
  };
`,
  sandbox
);

const run = (expr) => vm.runInContext(expr, sandbox);
const t0 = Date.now();
const el = () => ((Date.now() - t0) / 60000).toFixed(1);

run('wait(712100)');
console.log(`warm-up done at ${el()} min`);

// The P1 register program, then the paced OAM writes and mask.
const PALETTE = [0x0f, 0x16, 0x2a, 0x12, 0x0f, 0x28, 0x14, 0x02,
                 0x0f, 0x26, 0x1a, 0x31, 0x0f, 0x30, 0x27, 0x06];
run('cpuAccess(1, 2, 0)');
run('cpuAccess(0, 0, 0)');
run('cpuAccess(0, 1, 0)');
run('cpuAccess(0, 6, 0x3f)');
run('cpuAccess(0, 6, 0x00)');
for (const v of PALETTE) run(`cpuAccess(0, 7, ${v})`);
run('cpuAccess(0, 6, 0x20)');
run('cpuAccess(0, 6, 0x00)');
run('cpuAccess(0, 5, 0)');
run('cpuAccess(0, 5, 0)');
run('cpuAccess(0, 3, 0)');
for (const v of [90, 1, 0, 180]) {
  run(`cpuAccess(0, 4, ${v})`);
  run('wait(24)');
}
// Read OAM back, stepping OAMADDR by hand.
const back = [];
for (let i = 0; i < 4; i++) {
  run(`cpuAccess(0, 3, ${i})`);
  run('wait(24)');
  back.push(run('cpuAccess(1, 4, 0)'));
  run('wait(24)');
}
console.log(`OAM[0..4] read back: [${back}] (wrote [90,1,0,180]) at ${el()} min`);
run('cpuAccess(0, 3, 0)');
run('cpuAccess(0, 1, 0x1e)');

// Checkpoints: spr0_p and spr0_hit through the fetch and display lines.
const at = (tv, th) =>
  run(`(function(){
    while (!(readBits('vpos', 9) == ${tv} && readBits('hpos', 9) >= ${th})) { halfStep(); halfSteps++; }
    return 'vpos ${tv} hpos ' + readBits('hpos', 9) +
           ': spr0_p ' + readBits('spr0_p', 8) +
           ' spr0_hit ' + (isNodeHigh(nodenames['spr0_hit']) ? 1 : 0);
  })()`);
for (const [tv, th] of [[90, 250], [90, 300], [90, 330], [91, 1], [91, 60], [91, 120], [91, 178], [91, 200]]) {
  console.log(`${at(tv, th)} (${el()} min)`);
}
console.log(`done at ${el()} min, halfSteps ${run('halfSteps')}`);
