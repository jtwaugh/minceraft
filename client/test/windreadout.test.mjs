// Run: node --test client/test/windreadout.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { initSync, Wind } from '../pkg/game_web.js';
import { formatWind } from '../windreadout.js';

initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

test('zero wind is calm, with no NaN', () => {
  assert.equal(formatWind(0, 0, 0), 'wind calm 0.0 b/s (gust 0.00)');
  assert.ok(!formatWind(0, 0, NaN).includes('NaN'));
});

test('compass points toward where the wind blows', () => {
  assert.match(formatWind(0, -2, 0.2), /^wind N 2\.0 b\/s/);
  assert.match(formatWind(2, 0, 0.2), /^wind E /);
  assert.match(formatWind(0, 2, 0.2), /^wind S /);
  assert.match(formatWind(-2, 0, 0.2), /^wind W /);
  assert.match(formatWind(1, -1, 0.2), /^wind NE /);
  assert.match(formatWind(-1, -1, 0.2), /^wind NW /);
});

test('a strong wind prints speed and gust', () => {
  assert.equal(formatWind(6, 8, 1), 'wind SE 10.0 b/s (gust 1.00)');
});

test('roughness and multiplier print with 2 decimals', () => {
  assert.equal(formatWind(6, 8, 0.45, 0.62, 0.713), 'wind SE 10.0 b/s (gust 0.45) R 0.62 x0.71');
});

test('calm wind still shows roughness', () => {
  assert.equal(formatWind(0, 0, 0, 0.5, 1), 'wind calm 0.0 b/s (gust 0.00) R 0.50 x1.00');
});

test('non-finite or missing inputs print as 0, never NaN', () => {
  assert.equal(formatWind(1, 0, 0.1, NaN, Infinity), 'wind E 1.0 b/s (gust 0.10) R 0.00 x0.00');
  assert.equal(formatWind(1, 0, 0.1, undefined, 0.5), 'wind E 1.0 b/s (gust 0.10) R 0.00 x0.50');
  assert.ok(!formatWind(NaN, NaN, undefined, NaN, NaN).includes('NaN'));
});

test('real wasm Wind: R in [0, 1], multiplier in (0, 1]', () => {
  const w = new Wind(12345n);
  for (const [x, z] of [[0, 0], [12.5, -4.2], [-800, 300], [5000, -7000]]) {
    const r = w.roughness(x, z);
    const m = w.ground_multiplier(x, z);
    assert.ok(r >= 0 && r <= 1, `R ${r}`);
    assert.ok(m > 0 && m <= 1, `multiplier ${m}`);
    assert.match(formatWind(1, 0, 0.5, r, m), / R \d\.\d\d x\d\.\d\d$/);
  }
});
