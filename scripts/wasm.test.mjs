// The engine as the browser runs it (web/pkg, built by scripts/build-web.sh) must write the same
// G-code as EasyGCode itself, like the native golden tests: the WebAssembly build has its own
// floating-point library, so this checks the browser's numbers, not just the test machine's.
//
//   node --test scripts/wasm.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const engine = await import(join(root, 'web/pkg/easygcode_wasm.js'));
engine.initSync({ module: readFileSync(join(root, 'web/pkg/easygcode_wasm_bg.wasm')) });

const commands = (text) =>
  text
    .split('\n')
    .map((l) => l.split(';')[0].trim())
    .filter(Boolean);

const fixtures = join(root, 'fixtures/easygcode');
const cases = readdirSync(fixtures).filter((f) => f.endsWith('.project.json'));

test('there are golden cases', () => assert.ok(cases.length >= 6));

for (const file of cases) {
  const name = file.replace('.project.json', '');
  test(`${name}: the browser's engine writes EasyGCode's G-code`, () => {
    const project = readFileSync(join(fixtures, file), 'utf8');
    const want = commands(readFileSync(join(fixtures, `${name}.gcode`), 'utf8'));
    const got = commands(engine.gcode(project, 0));
    assert.equal(got.length, want.length);
    for (let i = 0; i < want.length; i++) assert.equal(got[i], want[i], `command ${i}`);
  });
}

test('a run gives the settings back complete, and the preview as typed arrays', () => {
  const r = engine.run(JSON.stringify({ kind: 'vase' }));
  const meta = JSON.parse(r.meta);
  assert.equal(meta.kind.id, 'vase');
  assert.equal(meta.settings.shape.circle.radius, 30);
  assert.equal(meta.parts.length, 1);
  assert.ok(r.parts[0].positions instanceof Float32Array);
  assert.equal(r.parts[0].positions.length, 3 * r.parts[0].feeds.length);
  assert.equal(r.parts[0].travel.length, r.parts[0].feeds.length);
});

test('bad input is a message, never a crash', () => {
  assert.match(JSON.parse(engine.run('{').meta).error, /isn't a project/);
  assert.throws(() => engine.gcode('{"kind":"vase","settings":{"height":-1}}', 0), /Fix the settings first/);
  assert.throws(() => engine.schema('nope'), /doesn't know/);
});
