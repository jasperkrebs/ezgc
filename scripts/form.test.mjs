// The settings window's value helpers (the window itself is covered by the smoke test):
//
//   node --test scripts/form.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { get, set } from '../web/js/form.js';

test('values at dotted keys', () => {
  const v = { shape: { kind: 'circle', circle: { radius: 30 } }, height: 60 };
  assert.equal(get(v, 'shape.circle.radius'), 30);
  assert.equal(get(v, 'shape.star.points'), undefined);
  const w = set(v, 'shape.circle.radius', 12);
  assert.equal(get(w, 'shape.circle.radius'), 12);
  assert.equal(get(v, 'shape.circle.radius'), 30, 'the original is untouched');
  assert.equal(get(set({}, 'a.b.c', true), 'a.b.c'), true);
});
