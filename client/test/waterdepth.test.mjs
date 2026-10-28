import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { WATER_DEPTH_GLSL } from '../../viewer/water.js';

const src = readFileSync(new URL('../scene.js', import.meta.url), 'utf8');

test('client shader embeds the shared water depth GLSL', () => {
  assert.match(src, /import \{[^}]*WATER_DEPTH_GLSL[^}]*\} from '\.\.\/viewer\/water\.js'/);
  assert.match(src, /\$\{WATER_DEPTH_GLSL\}/);
  assert.match(WATER_DEPTH_GLSL, /float waterColorFactor\(float depth\)/);
  assert.match(WATER_DEPTH_GLSL, /float waterAlphaScale\(float depth\)/);
});

test('client reads vertex byte 15 as attribute 7 and uses the depth mapping', () => {
  assert.match(src, /layout\(location=7\) in float depth;/);
  assert.match(src, /attr\(7, 1, gl\.UNSIGNED_BYTE, false, 15\)/);
  assert.match(src, /waterColorFactor\(wd\)/);
  assert.match(src, /waterAlphaScale\(wd\)/);
});

test('depth shading is off under water (waterFresnel is 0 there)', () => {
  assert.match(src, /float wd = vdepth \* waterFresnel;/);
});
