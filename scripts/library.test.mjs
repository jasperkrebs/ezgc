// The library (projects and printer profiles on the device) and its files, without a browser:
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
  };
}

const spiral = (name, settings = {}) => ({ app: 'ezgc', format: 1, kind: 'spiral', name, settings });
const printer = (name, model = 'g1') => ({ type: 'printer', name, settings: { model } });

test('projects are saved, listed newest first, loaded and removed', async () => {
  const { projects } = createLibrary(memoryStorage());
  const a = projects.save(spiral('A'));
  await new Promise((r) => setTimeout(r, 5));
  const b = projects.save(spiral('B'));
  assert.ok(a.id && b.id && a.id !== b.id);
  assert.deepEqual(projects.list().map((e) => e.name), ['B', 'A']);
  assert.equal(projects.load(a.id).name, 'A');
  projects.setCurrent(a.id);
  projects.remove(a.id);
  assert.equal(projects.load(a.id), null);
  assert.equal(projects.current(), null);
  assert.deepEqual(projects.list().map((e) => e.name), ['B']);
});

test('printers are their own collection', () => {
  const lib = createLibrary(memoryStorage());
  const g1 = lib.printers.save(printer('Ginger G1'));
  lib.projects.save(spiral('A'));
  assert.equal(lib.printers.list().length, 1);
  assert.equal(lib.printers.list()[0].kind, 'g1');
  assert.equal(lib.projects.list().length, 1);
  lib.printers.setCurrent(g1.id);
  assert.equal(lib.printers.current(), g1.id);
  assert.equal(lib.projects.current(), null);
  // A project isn't a printer and the other way round.
  assert.equal(lib.printers.load(lib.projects.list()[0].id), null);
});

test('a damaged item falls back to its previous version', () => {
  const store = memoryStorage();
  const { projects } = createLibrary(store);
  const p = projects.save(spiral('First'));
  projects.save({ ...p, name: 'Second' });
  store.setItem(`ezgc.projects.${p.id}`, '{ not json');
  assert.equal(projects.load(p.id).name, 'First');
});

test('a duplicate is a new item with its own id', () => {
  const { projects, printers } = createLibrary(memoryStorage());
  const p = projects.save(spiral('Tall', { height: 120 }));
  const copy = projects.duplicate(p.id);
  assert.notEqual(copy.id, p.id);
  assert.equal(copy.name, 'Tall (copy)');
  assert.equal(copy.settings.height, 120);
  const pr = printers.duplicate(printers.save(printer('P1P', 'p1p')).id);
  assert.equal(pr.settings.model, 'p1p');
  assert.equal(printers.list().length, 2);
});

test('a full store says so instead of throwing', () => {
  const store = memoryStorage();
  const lib = createLibrary(store);
  store.full = true;
  const p = lib.projects.save(spiral('Nowhere'));
  assert.equal(lib.ok, false);
  assert.ok(p.id);
});

test('files leave the bookkeeping behind, and come back in', () => {
  const lib = createLibrary(memoryStorage());
  const p = lib.projects.save(spiral('Shared', { height: 80 }));
  const file = JSON.parse(toFile(p));
  assert.equal(file.app, 'ezgc');
  assert.equal(file.id, undefined);
  assert.equal(file.modified, undefined);
  assert.equal(file.settings.height, 80);
  assert.equal(parseFile(toFile(p)).projects[0].name, 'Shared');
  const pr = lib.printers.save(printer('Mine', 'p1p'));
  const back = parseFile(toFile(pr));
  assert.deepEqual(back.projects, []);
  assert.equal(back.printers[0].settings.model, 'p1p');
  const all = parseFile(bundle([p, lib.projects.save(spiral('Other'))], [pr]));
  assert.deepEqual(all.projects.map((x) => x.name), ['Shared', 'Other']);
  assert.deepEqual(all.printers.map((x) => x.name), ['Mine']);
});

test('wrong files are messages', () => {
  assert.match(parseFile('nope').error, /isn't JSON/);
  assert.match(parseFile('[]').error, /isn't an ezgc file/);
  assert.match(parseFile('{"app":"easygcode","version":1,"settings":{}}').error, /another app \(easygcode\)/);
  assert.match(parseFile('{"projects":[]}').error, /empty/);
  assert.match(parseFile('{"name":"x"}').error, /isn't an ezgc project or printer/);
});
