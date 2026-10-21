import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { uiScale, findTexture, textureSize, pixelData, textureCanvas } from '../uipx.js';

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
