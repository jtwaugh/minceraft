import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { uiScale, interfaceScale, BELT_TEXELS_W, BELT_MAX_WIDTH_FRACTION, findTexture, textureSize, pixelData, textureCanvas } from '../uipx.js';

// Needs viewer/textures.json (./build.sh writes it).
const set = JSON.parse(readFileSync(new URL('../../viewer/textures.json', import.meta.url), 'utf8'));

test('uiScale is an integer >= 1 for any viewport', () => {
  for (const [w, h] of [[0, 0], [1, 1], [-5, 10], [100, 0], [319, 179], [320, 180], [1920, 1080], [NaN, 5], [Infinity, Infinity]]) {
    const s = uiScale(w, h, 320, 180);
    assert.ok(Number.isInteger(s) && s >= 1, `${w}x${h} -> ${s}`);
  }
  assert.equal(uiScale(1920, 1080, 320, 180), 6);
  assert.equal(uiScale(1920, 700, 320, 180), 3);
});

const HEARTS_TEXELS_W = 20 / 2 * 8 + (20 / 2 - 1) * 2;

test('interfaceScale keeps the belt within its width share and the hearts on screen', () => {
  assert.equal(BELT_TEXELS_W, 58);
  for (const [w, h] of [[390, 844], [1366, 768], [1920, 1080], [3440, 1440], [0, 0], [100, 50]]) {
    const s = interfaceScale(w, h);
    assert.ok(Number.isInteger(s) && s >= 1, `${w}x${h} -> ${s}`);
    if (w >= 390) {
      assert.ok(BELT_TEXELS_W * s <= w * BELT_MAX_WIDTH_FRACTION, `belt ${w}x${h}`);
      assert.ok(HEARTS_TEXELS_W * s <= w, `hearts ${w}x${h}`);
    }
    if (s > 1) assert.ok(BELT_TEXELS_W * s <= w * BELT_MAX_WIDTH_FRACTION && s <= uiScale(w, h, 100, 60));
  }
  assert.equal(interfaceScale(390, 844), 3);
  assert.equal(interfaceScale(1366, 768), 11);
});

test('interfaceScale never decreases as the viewport grows', () => {
  for (const h of [0, 50, 300, 844, 1440]) {
    let prev = 0;
    for (let w = 0; w <= 4000; w += 10) {
      const s = interfaceScale(w, h);
      assert.ok(s >= prev, `${w}x${h}`);
      prev = s;
    }
  }
  for (const w of [0, 390, 1920]) {
    let prev = 0;
    for (let h = 0; h <= 3000; h += 10) {
      const s = interfaceScale(w, h);
      assert.ok(s >= prev, `${w}x${h}`);
      prev = s;
    }
  }
});

test('texture lookup finds icons and non-8x8 tiles, null when unknown', () => {
  assert.ok(findTexture(set, 'sword'));
  assert.deepEqual(textureSize(set, 'slot'), { w: 10, h: 10 });
  assert.deepEqual(textureSize(set, 'digit_0'), { w: 4, h: 6 });
  assert.equal(findTexture(set, 'no_such_texture'), null);
  assert.equal(textureSize(set, 'no_such_texture'), null);
  assert.equal(textureCanvas(set, 'no_such_texture', {}), null);
});

test('pixelData maps palette indices to RGBA with . transparent', () => {
  const d = pixelData(['0.', 'f0'], { 0: '#102030', f: '#ff8000' });
  assert.deepEqual([...d], [16, 32, 48, 255, 0, 0, 0, 0, 255, 128, 0, 255, 16, 32, 48, 255]);
});
