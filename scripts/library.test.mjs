// The project library and settings helpers, without a browser:
//
//   node --test scripts/library.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bundle, createLibrary, parseFile, toFile } from '../web/js/library.js';

/** localStorage, in memory. `full` makes every write fail, like a full or private-mode store. */
function memoryStorage() {
  const m = new Map();
  return {
    full: false,
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem(k, v) {
      if (this.full) throw new Error('QuotaExceededError');
      m.set(k, String(v));
    },
    removeItem: (k) => m.delete(k),
    keys: () => [...m.keys()],
  };
}

const vase = (name, settings = {}) => ({ app: 'easygcode', format: 2, kind: 'vase', name, settings, printer: {} });

test('projects are saved, listed newest first, loaded and removed', async () => {
  const lib = createLibrary(memoryStorage());
  const a = lib.save(vase('A'));
  await new Promise((r) => setTimeout(r, 5));
  const b = lib.save(vase('B'));
  assert.ok(a.id && b.id && a.id !== b.id);
  assert.deepEqual(lib.list().map((e) => e.name), ['B', 'A']);
  assert.equal(lib.load(a.id).name, 'A');
  lib.setCurrent(a.id);
  lib.remove(a.id);
  assert.equal(lib.load(a.id), null);
  assert.equal(lib.current(), null);
  assert.deepEqual(lib.list().map((e) => e.name), ['B']);
});

test('a damaged project falls back to its previous version', () => {
  const store = memoryStorage();
  const lib = createLibrary(store);
  const p = lib.save(vase('First'));
  lib.save({ ...p, name: 'Second' });
  store.setItem(`easygcode2.p.${p.id}`, '{ not json');
  assert.equal(lib.load(p.id).name, 'First');
});

test('a duplicate is a new project with its own id', () => {
  const lib = createLibrary(memoryStorage());
  const p = lib.save(vase('Tall', { height: 120 }));
  const copy = lib.duplicate(p.id);
  assert.notEqual(copy.id, p.id);
  assert.equal(copy.name, 'Tall (copy)');
  assert.equal(copy.settings.height, 120);
  assert.equal(lib.list().length, 2);
});

test('a full store says so instead of throwing', () => {
  const store = memoryStorage();
  const lib = createLibrary(store);
  store.full = true;
  const p = lib.save(vase('Nowhere'));
  assert.equal(lib.ok, false);
  assert.ok(p.id);
});

test('files leave the bookkeeping behind, and come back in', () => {
  const lib = createLibrary(memoryStorage());
  const p = lib.save(vase('Shared', { height: 80 }));
  const file = JSON.parse(toFile(p));
  assert.equal(file.id, undefined);
  assert.equal(file.modified, undefined);
  assert.equal(file.settings.height, 80);
  const back = parseFile(toFile(p));
  assert.equal(back.projects[0].name, 'Shared');
  const all = parseFile(bundle([p, lib.save(vase('Other'))]));
  assert.deepEqual(all.projects.map((x) => x.name), ['Shared', 'Other']);
});

test('wrong files are messages', () => {
  assert.match(parseFile('nope').error, /isn't JSON/);
  assert.match(parseFile('[]').error, /no kind/);
  assert.match(parseFile('{"app":"easygcode","version":1,"settings":{}}').error, /EasyGCode 1/);
  assert.match(parseFile('{"projects":[]}').error, /no projects/);
});
