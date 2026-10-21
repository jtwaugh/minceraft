// The near plane must not reach into a block the player can stand against,
// and the underwater look must differ from the dry one. No wasm needed.
// Run: node --test client/test/view.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { NEAR, nearPlaneCorners, basis, look, farPlane, FOG_END, fogAt, skyInputs, SUN_DIR } from '../view.js';
import { fogAmount } from '../../viewer/fog.js';

const FOV = 70 * Math.PI / 180; // data/input.toml look.fov_degrees
const HALF_WIDTH = 0.3;         // data/player.toml width / 2
const HEADROOM = 1.8 - 1.62;    // height - eye_height

// Every look direction, every 5 degrees of yaw and pitch.
function* looks() {
  for (let y = -180; y < 180; y += 5) {
    for (let p = -90; p <= 90; p += 5) yield [y * Math.PI / 180, p * Math.PI / 180];
  }
}

// How far the near plane reaches past the eye into a solid half-space along
// `axis` (0 x, 1 y), solid from `eye[axis] + gap` on.
function worstReach(axis, gap, aspect, near) {
  const eye = [0, 0, 0];
  let worst = -Infinity;
  for (const [yaw, pitch] of looks()) {
    for (const c of nearPlaneCorners(eye, yaw, pitch, FOV, aspect, near)) worst = Math.max(worst, c[axis] - gap);
  }
  return worst; // > 0: a corner is inside the block
}

for (const aspect of [4 / 3, 16 / 9, 21 / 9]) {
  test(`near plane stays out of a wall the player touches, aspect ${aspect.toFixed(2)}`, () => {
    assert.ok(worstReach(0, HALF_WIDTH, aspect, NEAR) < 0);
  });
  test(`near plane stays out of the ceiling over the player's head, aspect ${aspect.toFixed(2)}`, () => {
    assert.ok(worstReach(1, HEADROOM, aspect, NEAR) < 0);
  });
}

test('the old 0.3 near plane did reach into both', () => {
  assert.ok(worstReach(0, HALF_WIDTH, 16 / 9, 0.3) > 0);
  assert.ok(worstReach(1, HEADROOM, 16 / 9, 0.3) > 0);
});

test('the basis is orthonormal and matches camera.rs at yaw 0', () => {
  const { f, r, u } = basis(0, 0);
  assert.deepEqual(f.map((v) => Math.round(v) + 0), [0, 0, -1]);
  assert.deepEqual(r.map((v) => Math.round(v) + 0), [1, 0, 0]);
  assert.deepEqual(u.map((v) => Math.round(v) + 0), [0, 1, 0]);
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  for (const [yaw, pitch] of [[0.4, 0.9], [-2, -1.2]]) {
    const b = basis(yaw, pitch);
    for (const v of [b.f, b.r, b.u]) assert.ok(Math.abs(dot(v, v) - 1) < 1e-9);
    assert.ok(Math.abs(dot(b.f, b.r)) < 1e-9 && Math.abs(dot(b.f, b.u)) < 1e-9 && Math.abs(dot(b.r, b.u)) < 1e-9);
  }
});

test('the far plane is past the solid fog', () => {
  assert.ok(farPlane(100) > 100 * FOG_END);
});

test('dry look is the sky fog, untinted; underwater is short blue fog and a tint', () => {
  const sky = [0.6, 0.7, 0.9], water = [47, 95, 200];
  const dry = look({ underwater: false, skyColor: sky, fogFar: 100, water });
  assert.deepEqual(dry.fogColor, sky);
  assert.deepEqual(dry.tint, [1, 1, 1]);
  assert.ok(dry.fogStart > 0 && dry.fogEnd > dry.fogStart);
  const wet = look({ underwater: true, skyColor: sky, fogFar: 100, water });
  assert.equal(wet.fogStart, 0);
  assert.ok(wet.fogEnd < dry.fogEnd);
  assert.ok(wet.tint[2] === 1 && wet.tint[0] < 1 && wet.tint[1] < 1);
  assert.ok(wet.fogColor[2] > wet.fogColor[0]);
});

test('dry fog is the viewer curve, solid at FOG_END x fogFar; underwater stays linear', () => {
  const sky = [0.6, 0.7, 0.9], water = [47, 95, 200];
  const dry = look({ underwater: false, skyColor: sky, fogFar: 100, water });
  for (const d of [0, 40, 60, 90, 115, 200]) {
    assert.equal(fogAt(dry, d), fogAmount(d, 100 * FOG_END));
  }
  assert.equal(fogAt(dry, 100 * FOG_END), 1);
  const wet = look({ underwater: true, skyColor: sky, fogFar: 100, water });
  assert.equal(wet.fogFar, 0);
  assert.equal(fogAt(wet, 9), 0.5);
  assert.equal(fogAt(wet, 30), 1);
});

test('the sky pass runs above water with the biome colour, and not under water', () => {
  const base = { skyColor: [0.6, 0.7, 0.9], fogFar: 100, water: [40, 60, 200] };
  const up = skyInputs(look({ ...base, underwater: false }), 0.3, 0.1, FOV, 16 / 9);
  assert.deepEqual(up.base, base.skyColor);
  assert.deepEqual(up.sunDir, SUN_DIR);
  assert.ok(Math.abs(Math.hypot(...SUN_DIR) - 1) < 1e-12);
  assert.ok(Math.abs(up.scale[0] / up.scale[1] - 16 / 9) < 1e-12);
  assert.deepEqual(up.fwd, basis(0.3, 0.1).f);
  assert.equal(skyInputs(look({ ...base, underwater: true }), 0.3, 0.1, FOV, 16 / 9), null);
});
