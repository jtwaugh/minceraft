import test from 'node:test';
import assert from 'node:assert/strict';
import { parseBlend } from '../blend.js';

test('blend setting: clamps to 0..7', () => {
  assert.equal(parseBlend('-3'), 0);
  assert.equal(parseBlend('99'), 7);
  assert.equal(parseBlend('0'), 0);
  assert.equal(parseBlend('7'), 7);
  assert.equal(parseBlend('4'), 4);
  assert.equal(parseBlend('2.6'), 3);
});

test('blend setting: non-numbers and unset fall back to the default', () => {
  for (const v of [null, undefined, '', ' ', 'abc', 'NaN', 'Infinity']) assert.equal(parseBlend(v), undefined);
});
