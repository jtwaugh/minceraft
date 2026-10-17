import test from 'node:test';
import assert from 'node:assert/strict';
import { waterFrame } from '../waterframe.js';

test('water cycles through 4 frames, 300 ms each', () => {
  const frames = [0, 299, 300, 599, 600, 900, 1199, 1200, 1500].map((t) => waterFrame(t, false));
  assert.deepEqual(frames, [0, 0, 1, 1, 2, 3, 3, 0, 1]);
});

test('water stays on frame 0 under reduced motion', () => {
  for (const t of [0, 300, 650, 905, 123456]) assert.equal(waterFrame(t, true), 0);
});
