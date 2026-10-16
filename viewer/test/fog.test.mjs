import test from 'node:test';
import assert from 'node:assert/strict';
import { fogAmount, FOG_NEAR } from '../fog.js';
import { VIEW } from '../flyover/plan.js';

test('fog is 0 at the near end and exactly 1 at VIEW.fogEnd', () => {
  assert.equal(fogAmount(0, VIEW.fogEnd), 0);
  assert.equal(fogAmount(VIEW.fogEnd * FOG_NEAR, VIEW.fogEnd), 0);
  assert.equal(fogAmount(VIEW.fogEnd, VIEW.fogEnd), 1);
  assert.equal(fogAmount(VIEW.fogEnd * 2, VIEW.fogEnd), 1);
});

test('fog rises monotonically between the ends', () => {
  let prev = 0;
  for (let d = 0; d <= VIEW.fogEnd; d += 4) {
    const f = fogAmount(d, VIEW.fogEnd);
    assert.ok(f >= prev);
    prev = f;
  }
});

test('equal radial distance gives equal fog, whatever the direction', () => {
  const eye = [10, 70, -20];
  const dist = (p) => Math.hypot(p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]);
  const a = [eye[0] + 150, eye[1], eye[2]];
  const b = [eye[0], eye[1], eye[2] - 150];
  const c = [eye[0] + 90, eye[1] + 120, eye[2]];
  assert.equal(fogAmount(dist(a), VIEW.fogEnd), fogAmount(dist(b), VIEW.fogEnd));
  assert.ok(Math.abs(fogAmount(dist(c), VIEW.fogEnd) - fogAmount(150, VIEW.fogEnd)) < 1e-12);
});
