import test from 'node:test';
import assert from 'node:assert/strict';
import { visibilitySchedule, VIEW, SEA, CHUNK, TREE_CLEARANCE } from '../flyover/plan.js';

// survey at sea level over x, z in [-512, 512], with optional peaks
function grid(peaks = []) {
  const step = 16, nx = 65, nz = 65, x0 = -512, z0 = -512;
  const top = new Uint8Array(nx * nz).fill(SEA);
  for (const [px, pz, h] of peaks) top[((pz - z0) / step) * nx + (px - x0) / step] = h;
  return { x0, z0, step, nx, nz, top };
}
// camera standing still at `eye`, looking along `dir`
const still = (eye, dir) => ({ duration: 1, at: () => ({ eye, target: eye.map((v, i) => v + dir[i] * 50) }) });
const has = (s, cx, cz) => s.some((e) => e.cx === cx && e.cz === cz);
const opts = { near: 0, margin: 0 };

test('a peak in view schedules a chunk the flat ground would not', () => {
  // camera high up looking level: the ground of chunk 2 is under the bottom of
  // the view, but a peak in it reaches the frustum
  const eye = [0, 200, 32], dir = [1, 0, 0];
  const flat = visibilitySchedule(still(eye, dir), grid(), opts);
  const peak = visibilitySchedule(still(eye, dir), grid([[144, 16, 110]]), opts);
  assert.ok(!has(flat, 2, 0));
  assert.ok(has(peak, 2, 0));
});

test('tree clearance lifts the box into view', () => {
  // camera at sea level looking up: only the tree-top part of flat ground is in the frustum
  const eye = [0, SEA, 32], dir = [0.9, 0.436, 0];
  const s = visibilitySchedule(still(eye, dir), grid(), opts);
  assert.ok(has(s, 1, 0));
  assert.ok(TREE_CLEARANCE > 0);
});

test('chunks outside the frustum or beyond the fog are not scheduled', () => {
  const eye = [0, 90, 32], dir = [1, 0, 0];
  const s = visibilitySchedule(still(eye, dir), grid(), opts);
  assert.ok(!has(s, -3, 0), 'behind the camera');
  assert.ok(!has(s, 1, 4), 'far to the side');
  assert.ok(!has(s, 5, 0), 'beyond fogEnd');
  assert.ok(5 * CHUNK > VIEW.fogEnd);
  assert.ok(has(s, 1, 0));
});

test('the distance limit is radial in 3D', () => {
  assert.ok(has(visibilitySchedule(still([0, 90, 32], [1, 0, 0]), grid(), opts), 2, 0));
  assert.ok(!has(visibilitySchedule(still([0, 260, 32], [0.64, -0.77, 0]), grid(), opts), 2, 0));
});

test('chunks first seen from afar are half detail until the swap', () => {
  // fly level along +x at 20 blocks per second, from inside chunk 0
  const path = { duration: 12, at: (t) => { const eye = [32 + 20 * t, 90, 32]; return { eye, target: [eye[0] + 50, 90, 32] }; } };
  const s = visibilitySchedule(path, grid(), { near: 0, margin: 0 });
  const e = (cx) => s.find((c) => c.cx === cx && c.cz === 0);
  assert.equal(e(0).half, false);
  const far = s.filter((c) => c.half);
  assert.ok(far.length > 0);
  for (const c of far) {
    // swapped to full detail after it first shows, with time to load before the camera is within FULL_DETAIL
    assert.ok(c.full > c.first && c.full < Infinity || c.full === Infinity);
  }
  const swapped = far.find((c) => c.full < Infinity);
  assert.ok(swapped, 'a far-first chunk the camera reaches');
  assert.ok(swapped.full - swapped.first >= 1);
  assert.ok(!s.some((c) => c.half && c.first === 0 && c.cx < 2));
});
