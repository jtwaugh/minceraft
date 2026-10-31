import test from 'node:test';
import assert from 'node:assert/strict';
import { swayOffset, swayPhase, trunkLean, SWAY_AMPLITUDE, TRUNK_SWAY_AMPLITUDE } from '../sway.js';

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

test('trunk lean: zero at weight 0, grows with weight, same wave as plants', () => {
  assert.ok(TRUNK_SWAY_AMPLITUDE > 0 && TRUNK_SWAY_AMPLITUDE < SWAY_AMPLITUDE);
  const still = swayPhase(123456, true);
  for (const [x, z, p] of [[0, 0, still], [13.5, -7, 2], [-100, 55, 5]]) {
    assert.ok(trunkLean(0, x, z, p).every((d) => d === 0));
    const mag = (w) => Math.hypot(...trunkLean(w, x, z, p));
    assert.ok(mag(0.25) < mag(0.5) && mag(0.5) < mag(1));
    const k = TRUNK_SWAY_AMPLITUDE / SWAY_AMPLITUDE;
    const [lx, lz] = trunkLean(1, x, z, p), [px, pz] = swayOffset(1, x, z, p);
    assert.ok(Math.abs(lx - px * k) < 1e-12 && Math.abs(lz - pz * k) < 1e-12);
  }
});

test('trunk lean: at weight 1 never exceeds half a texel (0.03 blocks)', () => {
  let max = 0;
  for (let x = -40; x <= 40; x += 0.5) {
    for (let z = -40; z <= 40; z += 0.5) {
      for (let p = 0; p < 2 * Math.PI; p += 0.25) max = Math.max(max, Math.hypot(...trunkLean(1, x, z, p)));
    }
  }
  assert.ok(max <= 0.03, `max lean ${max}`);
  assert.ok(Math.SQRT2 * TRUNK_SWAY_AMPLITUDE <= 0.03); // bound on any (x, z, phase)
});
