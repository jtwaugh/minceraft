import test from 'node:test';
import assert from 'node:assert/strict';
import { swayOffset, swayPhase, SWAY_AMPLITUDE } from '../sway.js';

test('sway: zero at weight 0, bounded by the amplitude', () => {
  for (const [x, z, p] of [[0, 0, 0], [13.5, -7, 2], [-100, 55, 5]]) {
    assert.ok(swayOffset(0, x, z, p).every((d) => d === 0));
    const [dx, dz] = swayOffset(1, x, z, p);
    assert.ok(Math.abs(dx) <= SWAY_AMPLITUDE && Math.abs(dz) <= SWAY_AMPLITUDE);
  }
});

test('sway: varies with time and world position, scales with weight', () => {
  assert.notDeepEqual(swayOffset(1, 3, 4, 0), swayOffset(1, 3, 4, 1));
  assert.notDeepEqual(swayOffset(1, 3, 4, 0), swayOffset(1, 9, 4, 0));
  const [fx] = swayOffset(1, 3, 4, 0.5), [hx] = swayOffset(0.5, 3, 4, 0.5);
  assert.ok(Math.abs(hx - fx / 2) < 1e-12);
});

test('sway phase: wraps, and is still when motion is reduced', () => {
  assert.equal(swayPhase(123456, true), 0);
  const p = swayPhase(1e9, false);
  assert.ok(p >= 0 && p < 2 * Math.PI);
});
