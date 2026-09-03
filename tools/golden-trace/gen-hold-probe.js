#!/usr/bin/env node
// The reference's own truth about the suppressed conflicts at init:
// instrument getNodeValue to log every suppression (group size, pulls,
// charge, outcome), run initChip exactly as gen.js does, then report
// gnd/pwr stored state, the on-count of rail-gated transistors, and
// io_db7's level. Minutes, not hours: init only.

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

  // Instrument: wrap getNodeValue to log the suppressed path.
  holdLog = [];
  var SPECIAL = [359, 566, 691, 871, 870, 864, 856, 818];
  var orig = getNodeValue;
  getNodeValue = function () {
    var gnd = arrayContains(group, ngnd);
    var pwr = arrayContains(group, npwr);
    if (gnd && pwr) {
      var marked = false;
      for (var i = 0; i < SPECIAL.length; i++) if (arrayContains(group, SPECIAL[i])) marked = true;
      if (marked) {
        var pu = 0, pd = 0, hi = 0, lo = 0;
        for (var j in group) {
          var nn = group[j];
          if (nn == ngnd || nn == npwr) continue;
          var n = nodes[nn];
          if (n.pullup) pu++;
          if (n.pulldown) pd++;
          if (n.state) hi++; else lo++;
        }
        var out = orig();
        holdLog.push([group.length, pu, pd, hi, lo, out ? 1 : 0]);
        return out;
      }
    }
    return orig();
  };

  for (var nn in nodes) { nodes[nn].state = false; nodes[nn].float = true; }
  nodes[ngnd].state = false; nodes[ngnd].float = false;
  nodes[npwr].state = true;  nodes[npwr].float = false;
  for (var tn in transistors) transistors[tn].on = (transistors[tn].gate == npwr);
  setLow('res');
  setLow('clk0');
  setHigh('io_ce');
  setHigh('int');
  recalcNodeList(allNodes());
  for (var i = 0; i < 4; i++) { setHigh('clk0'); setLow('clk0'); }
  setHigh('res');

  gndOn = 0; pwrOn = 0;
  for (var t in transistors) {
    var tr = transistors[t];
    if (tr.gate == ngnd && tr.on) gndOn++;
    if (tr.gate == npwr && tr.on) pwrOn++;
  }
  summary = 'holds=' + holdLog.length +
    ' gnd.state=' + (nodes[ngnd].state ? 1 : 0) +
    ' pwr.state=' + (nodes[npwr].state ? 1 : 0) +
    ' gnd-gated-on=' + gndOn + ' pwr-gated-on=' + pwrOn +
    ' io_db7(27)=' + (nodes[27].state ? 1 : 0);
`,
  sandbox
);
const run = (e) => vm.runInContext(e, sandbox);
console.log(run('summary'));
const log = run('JSON.stringify(holdLog)');
const rows = JSON.parse(log);
const shapes = {};
for (const [size, pu, pd, hi, lo, out] of rows) {
  const k = `pu=${pu} pd=${pd} hi=${hi} lo=${lo} out=${out}`;
  shapes[k] = (shapes[k] || 0) + 1;
}
for (const [k, v] of Object.entries(shapes).sort((a, b) => b[1] - a[1]).slice(0, 15)) {
  console.log(`${String(v).padStart(4)}  ${k}`);
}
