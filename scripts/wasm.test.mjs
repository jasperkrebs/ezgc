// The engine as the browser runs it (web/pkg, built by scripts/build-web.sh) must write exactly
// the G-code of the snapshots (fixtures/snapshots/), like the native tests: the WebAssembly build
// is its own build, so this checks the browser's numbers, not just the test machine's.
//
//   node --test scripts/wasm.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const engine = await import(join(root, 'web/pkg/ezgc_wasm.js'));
engine.initSync({ module: readFileSync(join(root, 'web/pkg/ezgc_wasm_bg.wasm')) });

const snapshots = join(root, 'fixtures/snapshots');
const cases = readdirSync(snapshots)
  .filter((f) => f.endsWith('.project.json'))
  .map((f) => f.replace('.project.json', ''));
const read = (name) => readFileSync(join(snapshots, name), 'utf8');
const G1 = JSON.stringify({ model: 'g1' });

test('there are snapshots', () => assert.ok(cases.length >= 4));

for (const name of cases) {
  test(`${name}: the browser's engine writes the snapshot's G-code`, () => {
    const [project, printer] = [read(`${name}.project.json`), read(`${name}.printer.json`)];
    const parts = JSON.parse(engine.run(project, printer).meta).parts;
    assert.ok(parts.length > 0);
    parts.forEach((part, i) => {
      // The first part's snapshot is <case>.gcode, another's <case>.<its name>.gcode.
      const file = i === 0 ? `${name}.gcode` : `${name}.${part.name.toLowerCase().replace(/ /g, '-')}.gcode`;
      const got = engine.gcode(project, printer, i);
      const want = read(file);
      const lines = [got.split('\n'), want.split('\n')];
      const at = lines[0].findIndex((l, k) => l !== lines[1][k]);
      assert.equal(at, -1, `${file}: line ${at + 1}: ${lines[0][at]} (snapshot: ${lines[1][at]})`);
      assert.equal(got, want);
    });
  });
}

test('a run gives the settings back complete, and the preview as typed arrays', () => {
  const r = engine.run(JSON.stringify({ kind: 'spiral' }), G1);
  const meta = JSON.parse(r.meta);
  assert.equal(meta.kind.id, 'spiral');
  assert.equal(meta.settings.shape.circle.radius, 30);
  assert.equal(meta.printer.bed.width, 1000);
  assert.equal(meta.parts.length, 1);
  assert.ok(r.parts[0].positions instanceof Float32Array);
  assert.equal(r.parts[0].positions.length, 3 * r.parts[0].feeds.length);
  assert.equal(r.parts[0].travel.length, r.parts[0].feeds.length);
});

test('new printers come from the machines', () => {
  const machines = JSON.parse(engine.printerSchema()).machines.map((m) => m.id);
  assert.deepEqual(machines, ['g1', 'p1p']);
  assert.equal(JSON.parse(engine.newPrinter('p1p')).bed.width, 256);
});

test('bad input is a message, never a crash', () => {
  assert.match(JSON.parse(engine.run('{', G1).meta).error, /isn't a project/);
  assert.match(JSON.parse(engine.run('{"kind":"spiral"}', '{"model":"x"}').meta).error, /printer "x"/);
  assert.throws(() => engine.gcode('{"kind":"spiral","settings":{"height":-1}}', G1, 0), /Fix the settings first/);
  assert.throws(() => engine.schema('nope'), /doesn't know/);
  assert.throws(() => engine.newPrinter('nope'), /doesn't know/);
});
