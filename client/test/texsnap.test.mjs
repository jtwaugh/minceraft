import { test } from 'node:test';
import assert from 'node:assert/strict';
import { snapTexel, SNAP_GLSL } from '../texsnap.js';

const TILE = 16;
const FWS = [0.05, 0.1, 0.25, 0.5, 1, 2, 4];
const texel = (px, fw) => Math.floor(snapTexel(px, fw));

test("a face's far edge never snaps into the next tile", () => {
  for (const fw of FWS) {
    for (const d of [0.5, 0.25, 0.1, 0.03, 0.01, 0.001]) {
      const t = texel(TILE - d, fw);
      assert.ok(t >= 0 && t <= 15, `px ${TILE - d}, fw ${fw}: texel ${t}`);
    }
  }
});

test("a face's near edge stays in its tile", () => {
  for (const fw of FWS) {
    for (const px of [0, 0.001, 0.03, 0.25]) assert.equal(texel(px, fw), 0, `px ${px}, fw ${fw}`);
  }
});

test('inside a merged quad the snap crosses into the next block', () => {
  for (const fw of [0.05, 0.1, 0.25, 0.5]) {
    assert.equal(texel(TILE + 0.01, fw), TILE, `fw ${fw}`);
    assert.equal(texel(2 * TILE - 0.01, fw), 2 * TILE - 1, `fw ${fw}`);
  }
});

test('magnified, every fragment samples the texel it lies in', () => {
  for (const fw of [0.05, 0.1, 0.25, 0.5, 1]) {
    for (let px = 0; px < 2 * TILE; px += 0.0625 + 0.003) {
      assert.equal(texel(px, fw), Math.floor(px), `px ${px}, fw ${fw}`);
    }
  }
});

test('away from boundaries the snap lands on texel centres', () => {
  assert.equal(snapTexel(3.5, 0.1), 3.5);
  assert.equal(snapTexel(3.2, 0.1), 3.5);
  assert.equal(snapTexel(3.8, 0.1), 3.5);
});

test('the GLSL snippet defines snapTexel', () => {
  assert.match(SNAP_GLSL, /vec2 snapTexel\(vec2 px, vec2 fw\)/);
});
