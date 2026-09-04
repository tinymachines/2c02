// The reference's answer to P3's palette write-path question: the same
// paced and back-to-back $2007 writes the Rust probe makes
// (crates/v2c02-dots/examples/p3-write-probe.rs), run on the JS chip
// with the P1 world's warm-up and program, and read back paced. If the
// reference lands the bytes as written, rung 0 has a divergence; if it
// ORs the palette index in, the model does and silicon does not.
// Prints the rows. About 35 minutes (the warm-up is the cost).
'use strict';
const fs = require('fs');
const path = require('path');
const vm = require('vm');
const REF = path.join(__dirname, '..', '..', 'extern', 'visual2c02');
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

  // The P1 standard world's VRAM.
  vram = function (a) { a &= 0x3fff; return ((a >> 4) ^ a) & 0xff; };
  chr_ale = isNodeHigh(nodenames['ale']);
  chr_rd = isNodeHigh(nodenames['rd']);
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
    if (!chr_ale && ale) chr_addr = readBits('ab', 14);
    if (chr_rd && !rd) writeBits('db', 8, vram(chr_addr & 0x3fff));
    if (!chr_rd && rd) floatBits('db', 8);
    chr_ale = ale; chr_rd = rd;
  };
  halfSteps = 0;
  stepOnce = function () {
    if (isNodeHigh(nodenames['clk0'])) setLow('clk0'); else setHigh('clk0');
    handleChrBus();
    halfSteps++;
  };
  wait = function (n) { for (var i = 0; i < n; i++) stepOnce(); };
  sampled = 0;
  accessEdge = function (rw, reg, val, counter) {
    if (counter == 24) {
      writeBits('io_ab', 3, reg);
      if (rw) floatBits('io_db', 8); else writeBits('io_db', 8, val);
      var n = nodes[nodenames['io_rw']];
      if (rw) { n.pulldown = false; n.pullup = true; }
      else { n.pulldown = true; n.pullup = false; }
      recalcNodeList([nodenames['io_rw']]);
    }
    if (counter == 16) setLow('io_ce');
    if (counter == 1) { if (rw) sampled = readBits('io_db', 8); setHigh('io_ce'); }
  };
  cpuAccess = function (rw, reg, val) {
    for (var counter = 24; counter >= 1; counter--) { accessEdge(rw, reg, val, counter); stepOnce(); }
    floatBits('io_db', 8);
    return sampled;
  };
  W = function (reg, val) { cpuAccess(false, reg, val); };
  R = function (reg) { return cpuAccess(true, reg, 0); };
  hex = function (a) { return a.map(function (v) { return ('0' + v.toString(16)).slice(-2); }).join(' '); };
  readRow = function (lo, n) {
    W(6, 0x3f); wait(48); W(6, lo); wait(96);
    var out = [];
    for (var i = 0; i < n; i++) { out.push(R(7)); wait(48); }
    return out;
  };
  clearRow = function (lo) {
    W(6, 0x3f); wait(48); W(6, lo); wait(192);
    for (var i = 0; i < 4; i++) { W(7, 0); wait(48); }
    wait(192);
  };
`,
  sandbox
);

const run = (e) => vm.runInContext(e, sandbox);
const t0 = Date.now();
// The P1 warm-up and program (rendering stays off).
run('wait(712100)');
console.log('warm-up done in', ((Date.now() - t0) / 60000).toFixed(1), 'min');
run(`R(2); W(0, 0); W(1, 0); W(6, 0x3f); W(6, 0x00);`);
run(`[0x0f,0x16,0x2a,0x12,0x0f,0x28,0x14,0x02,0x0f,0x26,0x1a,0x31,0x0f,0x30,0x27,0x06].forEach(function (v) { W(7, v); });`);
run(`W(6, 0x20); W(6, 0x00); W(5, 0); W(5, 0);`);
console.log('standard palette as the reference holds it: ' + run('hex(readRow(0x00, 16))'));
const cases = [[0, 0x11], [24, 0x11], [96, 0x11], [24, 0x21]];
for (const [idle, row] of cases) {
  run(`clearRow(${row}); W(6, 0x3f); wait(48); W(6, ${row}); wait(96);`);
  run(`[0x21, 0x11, 0x01, 0x2a].forEach(function (v) { W(7, v); wait(${idle}); }); wait(192);`);
  console.log(`idle ${idle} / $3F${row.toString(16)} -> ` + run(`hex(readRow(${row}, 4))`) + ' (wrote 21 11 01 2a)');
}
console.log('done in', ((Date.now() - t0) / 60000).toFixed(1), 'min');
