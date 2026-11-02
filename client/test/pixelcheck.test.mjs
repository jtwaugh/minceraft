import test from 'node:test';
import assert from 'node:assert/strict';
import { pixelCheck, paletteColors, MAX_CROPS, CROP_PADDING, SHADES } from '../pixelcheck.js';

const palette = { 0: '#1a1a1a', 1: '#c8503c', 2: '#3c8c50' };
const GOOD = [200 * 0.8, 80 * 0.8, 60 * 0.8].map(Math.round);

function image(w, h, fill = GOOD) {
  const data = new Uint8Array(w * h * 4);
  for (let i = 0; i < w * h; i++) data.set([...fill, 255], i * 4);
  return { data, width: w, height: h, set(x, y, c) { data.set([...c, 255], (y * w + x) * 4); } };
}

test('palette times shade only: no seams, no crops', () => {
  const img = image(20, 20);
  img.set(3, 3, [60, 140, 80].map((v) => Math.round(v * SHADES[0])));
  assert.deepEqual(pixelCheck(img, { palette }), { seams: 0, offPalette: 0, nearBlack: 0, crops: [] });
});

test('palette accepts an array of triples', () => {
  assert.deepEqual(paletteColors([[1, 2, 3]]), [[1, 2, 3]]);
});

test('dark line and off-palette pixel are counted and cropped', () => {
  const img = image(60, 40);
  for (let x = 5; x < 15; x++) img.set(x, 30, [2, 2, 2]);
  img.set(50, 10, [10, 200, 250]);
  const r = pixelCheck(img, { palette });
  assert.equal(r.nearBlack, 10);
  assert.equal(r.offPalette, 1);
  assert.equal(r.seams, 11);
  assert.equal(r.crops.length, 2);
  const inside = (c, x, y) => x >= c.x && x < c.x + c.w && y >= c.y && y < c.y + c.h;
  assert.ok(r.crops.some((c) => inside(c, 5, 30) && inside(c, 14, 30)));
  assert.ok(r.crops.some((c) => inside(c, 50, 10)));
});

test('sky colour and skip mask are ignored', () => {
  const img = image(10, 10);
  img.set(1, 1, [100, 150, 255]);
  img.set(2, 2, [1, 1, 1]);
  const skip = new Uint8Array(100);
  skip[2 * 10 + 2] = 1;
  assert.equal(pixelCheck(img, { palette, sky: [100, 150, 255], skip }).seams, 0);
  assert.equal(pixelCheck(img, { palette }).seams, 2);
});

test('more regions than MAX_CROPS: exactly MAX_CROPS, biggest first', () => {
  const step = 4 * CROP_PADDING + 4, n = MAX_CROPS + 3;
  const img = image(step * n, 20);
  for (let k = 0; k < n; k++) {
    const size = k === n - 1 ? 3 : 1;
    for (let d = 0; d < size; d++) img.set(k * step + 2 + d, 10, [1, 1, 1]);
  }
  const r = pixelCheck(img, { palette });
  assert.equal(r.crops.length, MAX_CROPS);
  assert.equal(r.crops[0].x, Math.max(0, (n - 1) * step + 2 - CROP_PADDING));
});

test('crops are clamped to the image edges', () => {
  const img = image(20, 20);
  img.set(0, 0, [1, 1, 1]);
  img.set(19, 19, [1, 1, 1]);
  const r = pixelCheck(img, { palette });
  assert.deepEqual(r.crops.find((c) => c.x === 0), { x: 0, y: 0, w: 1 + CROP_PADDING, h: 1 + CROP_PADDING });
  assert.ok(r.crops.every((c) => c.x >= 0 && c.y >= 0 && c.x + c.w <= 20 && c.y + c.h <= 20));
});

test('overlapping crops merge', () => {
  const img = image(40, 20);
  img.set(10, 10, [1, 1, 1]);
  img.set(10 + CROP_PADDING + 2, 10, [1, 1, 1]);
  const r = pixelCheck(img, { palette });
  assert.equal(r.crops.length, 1);
  assert.equal(r.crops[0].w, CROP_PADDING * 2 + CROP_PADDING + 3);
});
