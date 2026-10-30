import test from 'node:test';
import assert from 'node:assert/strict';
import { median } from '../pool.js';

test('median of an odd count is the middle value', () => assert.equal(median([5, 1, 3]), 3));
test('median of an even count averages the middle two', () => assert.equal(median([4, 1, 3, 2]), 2.5));
test('median of an empty list is 0', () => assert.equal(median([]), 0));
test('median does not reorder its input', () => {
  const a = [3, 1, 2];
  median(a);
  assert.deepEqual(a, [3, 1, 2]);
});
