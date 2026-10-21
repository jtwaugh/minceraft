import test from 'node:test';
import assert from 'node:assert/strict';
import { skyColor, SKY_GLSL, ZENITH_SCALE, SUN_TINT_MAX, SKY_WARM } from '../sky.js';

const BASE = [0.5, 0.7, 1.0];
const SUN = (() => { const v = [-0.7, 0.16, -0.7], l = Math.hypot(...v); return v.map((c) => c / l); })();
const AWAY = [-SUN[0], 0, -SUN[2]];
const near = (a, b) => a.forEach((x, i) => assert.ok(Math.abs(x - b[i]) < 1e-9, `${a} vs ${b}`));

test('horizon away from the sun is the base colour', () => {
  near(skyColor(BASE, AWAY, SUN), BASE);
});

test('straight up is the deeper zenith colour', () => {
  near(skyColor(BASE, [0, 1, 0], SUN), BASE.map((c) => c * ZENITH_SCALE));
});

test('toward the sun is warmer than away from it', () => {
  const toward = skyColor(BASE, SUN, SUN);
  const away = skyColor(BASE, AWAY, SUN);
  assert.ok(toward[0] > away[0]);
  assert.ok(toward[2] < away[2]);
});

test('sun tint is capped at SUN_TINT_MAX', () => {
  const g = skyColor(BASE, SUN, SUN);
  const up = Math.max(SUN[1], 0);
  BASE.forEach((c, i) => {
    const grad = c * (1 - up * (1 - ZENITH_SCALE));
    near([g[i]], [grad + (SKY_WARM[i] - grad) * SUN_TINT_MAX]);
  });
});

test('GLSL snippet declares skyColor', () => {
  assert.match(SKY_GLSL, /vec3 skyColor\(vec3 base, vec3 dir, vec3 sunDir\)/);
});

test('GLSL snippet uses the same constants as the JS', () => {
  assert.ok(SKY_GLSL.includes(ZENITH_SCALE.toFixed(2)));
  assert.ok(SKY_GLSL.includes(SUN_TINT_MAX.toFixed(2)));
  assert.ok(SKY_GLSL.includes(`vec3(${SKY_WARM.map((v) => v.toFixed(2)).join(', ')})`));
});
