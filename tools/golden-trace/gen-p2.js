#!/usr/bin/env node
// The P2 oracle: the reference engine executing p2-schedule.json
// BLINDLY: every register access at its absolute half-step, every node
// dumped inside the schedule's windows, nothing else read or decided.
// The schedule was computed by the proven Rust engine (p2-schedule.rs),
// so both engines walk one trajectory and the dumped windows are the
// comparison. About three hours at the reference's pace; run detached.
//
// Usage: node gen-p2.js [--out FILE]

const fs = require('fs');
const path = require('path');
const vm = require('vm');
const crypto = require('crypto');

const REF = path.resolve(__dirname, '../../extern/visual2c02');
const schedText = fs.readFileSync(path.join(__dirname, 'p2-schedule.json'), 'utf8');
const sched = JSON.parse(schedText);
const schedSha = crypto.createHash('sha256').update(schedText).digest('hex').slice(0, 16);

function arg(name, dflt) {
  const i = process.argv.indexOf(name);
  return i === -1 ? dflt : process.argv[i + 1];
}
const outPath = path.resolve(arg('--out', path.join(__dirname, 'golden-2c02-p2.txt')));

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

  vram = function (a) {
    a &= 0x3fff;
    if (a >= 0x0010 && a <= 0x001f) return 0xff;
    if (a < 0x2000) return 0x00;
    var nt = a & 0x0fff;
    return (nt & 0x03ff) < 0x03c0 ? 0x01 : 0x00;
  };
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

  maxNode = nodes.length;
  dumpState = function () {
    var s = '';
    for (var i = 0; i < maxNode; i++) s += (nodes[i] !== undefined && isNodeHigh(i)) ? '1' : '0';
    return s;
  };
  OUT = [];
  halfSteps = 0;
  WINDOWS = ${JSON.stringify(sched.windows.map(w => [w[1], w[2]]))};
  inWindow = function (h) {
    for (var i = 0; i < WINDOWS.length; i++)
      if (h > WINDOWS[i][0] && h <= WINDOWS[i][1]) return true;
    return false;
  };
  stepOnce = function () {
    if (isNodeHigh(nodenames['clk0'])) setLow('clk0'); else setHigh('clk0');
    handleChrBus();
    halfSteps++;
    if (inWindow(halfSteps)) OUT.push(dumpState());
  };
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
    if (counter == 1) setHigh('io_ce');
  };
  cpuAccess = function (rw, reg, val) {
    for (var counter = 24; counter >= 1; counter--) {
      accessEdge(rw, reg, val, counter);
      stepOnce();
    }
    floatBits('io_db', 8);
  };
`,
  sandbox
);

const run = (e) => vm.runInContext(e, sandbox);
const t0 = Date.now();
const accesses = sched.accesses;
let ai = 0;
const end = sched.end;
let lastReport = 0;
while (run('halfSteps') < end) {
  const h = run('halfSteps');
  if (ai < accesses.length && accesses[ai][0] === h) {
    const [, rw, reg, val] = accesses[ai];
    run(`cpuAccess(${rw}, ${reg}, ${val})`);
    ai++;
  } else {
    run('stepOnce()');
  }
  if (h - lastReport >= 50000) {
    lastReport = h;
    process.stderr.write(`\r${h}/${end} (${((Date.now() - t0) / 60000).toFixed(1)} min)`);
  }
}
process.stderr.write('\n');
if (ai !== accesses.length) {
  console.error(`SCHEDULE DESYNC: ${ai} of ${accesses.length} accesses fired`);
  process.exit(1);
}
const out = run('OUT');
const lines = [`2c02 p2 golden: schedule ${schedSha} nodes ${run('maxNode')} states ${out.length}`];
fs.writeFileSync(outPath, lines.concat(out).join('\n') + '\n');
console.log(`wrote ${outPath} (${out.length} states)`);
