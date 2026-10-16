// Pure-JS checks for the frame statistics.
// Run: node --test client/test/frametimes.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { FrameTimes } from '../frametimes.js';

test('frame times report nearest-rank percentiles over the window', () => {
  const f = new FrameTimes(100);
  for (let i = 1; i <= 100; i++) f.push(i);
  assert.equal(f.percentile(50), 50);
  assert.equal(f.percentile(99), 99);
  f.push(1000); // overwrites the oldest
  assert.equal(f.count, 100);
  assert.equal(f.percentile(100), 1000);
  assert.equal(new FrameTimes().percentile(99), 0);
});
