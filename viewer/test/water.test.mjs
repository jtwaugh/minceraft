import test from 'node:test';
import assert from 'node:assert/strict';
import { fresnel, FRESNEL_F0, waterColorFactor, waterAlphaScale, WATER_DEPTH_CAP, WATER_DEEP_DARKEN, WATER_DEEP_ALPHA, WATER_DEPTH_GLSL } from '../water.js';

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

test('depth 0 and 1 leave colour and alpha unchanged', () => {
  for (const d of [0, 1]) {
    assert.equal(waterColorFactor(d), 1);
    assert.equal(waterAlphaScale(d), 1);
  }
});

test('deeper water is darker and more opaque, monotonically', () => {
  let c = 1, a = 1;
  for (let d = 0; d <= 255; d++) {
    assert.ok(waterColorFactor(d) <= c);
    assert.ok(waterAlphaScale(d) >= a);
    c = waterColorFactor(d);
    a = waterAlphaScale(d);
  }
  assert.ok(waterColorFactor(10) < 1 && waterAlphaScale(10) > 1);
});

test('depth is capped and the factors stay in bounds', () => {
  assert.equal(waterColorFactor(WATER_DEPTH_CAP), WATER_DEEP_DARKEN);
  assert.equal(waterAlphaScale(WATER_DEPTH_CAP), WATER_DEEP_ALPHA);
  assert.equal(waterColorFactor(255), waterColorFactor(WATER_DEPTH_CAP));
  assert.equal(waterAlphaScale(255), waterAlphaScale(WATER_DEPTH_CAP));
  for (let d = 0; d <= 255; d++) {
    assert.ok(waterColorFactor(d) >= WATER_DEEP_DARKEN && waterColorFactor(d) <= 1);
  }
});

test('GLSL carries the same constants', () => {
  assert.ok(WATER_DEPTH_GLSL.includes(String(WATER_DEPTH_CAP - 1)));
  assert.ok(WATER_DEPTH_GLSL.includes((1 - WATER_DEEP_DARKEN).toFixed(3)));
  assert.ok(WATER_DEPTH_GLSL.includes((WATER_DEEP_ALPHA - 1).toFixed(3)));
});
