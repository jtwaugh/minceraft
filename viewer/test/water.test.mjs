import test from 'node:test';
import assert from 'node:assert/strict';
import { fresnel, FRESNEL_F0 } from '../water.js';

test('fresnel is 1 at grazing angles and small looking straight down', () => {
  assert.equal(fresnel(0), 1);
  assert.equal(fresnel(-1), FRESNEL_F0);
  assert.ok(fresnel(1) < 0.05);
});

test('fresnel falls monotonically with the view angle and ignores up/down', () => {
  let prev = 1;
  for (let y = 0; y <= 1; y += 0.05) {
    const f = fresnel(y);
    assert.ok(f <= prev);
    assert.equal(f, fresnel(-y));
    prev = f;
  }
});
