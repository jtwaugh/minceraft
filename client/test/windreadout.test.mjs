// Run: node --test client/test/windreadout.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { formatWind } from '../windreadout.js';

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
