import test from 'node:test';
import assert from 'node:assert/strict';
import { buildLevels, tileFor, NAMES } from '../tiles.js';
import { B } from '../../viewer/mesh.js';

const S = 16;
// One leaf-like layer: scattered opaque tinted texels (colour 0, tint grey), rest clear.
function leafLayer() {
  const colour = new Uint8Array(S * S * 4);
  const grey = new Uint8Array(S * S);
  for (let i = 0; i < S * S; i++) {
    if ((i * 7 + (i >> 3)) % 5 < 3) { colour[i * 4 + 3] = 255; grey[i] = 200; }
  }
  return { colour, grey };
}

test('coverage share at 0.5 alpha matches level 0 at every level', () => {
  const { colour, grey } = leafLayer();
  const { colourLevels } = buildLevels(colour, grey, 1);
  const share = (d) => { let n = 0; for (let i = 3; i < d.length; i += 4) if (d[i] >= 128) n++; return n / (d.length / 4); };
  const s0 = share(colourLevels[0]);
  assert.equal(colourLevels.length, 5);
  for (let l = 1; l < 5; l++) {
    const px = (S >> l) ** 2;
    assert.ok(Math.abs(share(colourLevels[l]) - s0) <= 0.5 / px + 1e-9, `level ${l}`);
  }
});

test('tint / alpha is unchanged at every level for opaque tinted texels', () => {
  const colour = new Uint8Array(S * S * 4);
  const grey = new Uint8Array(S * S).fill(200);
  for (let i = 0; i < S * S; i++) colour[i * 4 + 3] = 255;
  const { colourLevels, tintLevels } = buildLevels(colour, grey, 1);
  colourLevels.forEach((c, l) => tintLevels[l].forEach((t, i) => {
    assert.equal(c[i * 4 + 3], 255);
    assert.equal(t, 200);
  }));
});

test('tint scales with alpha on partial coverage', () => {
  const { colour, grey } = leafLayer();
  const { colourLevels, tintLevels } = buildLevels(colour, grey, 1);
  for (let l = 1; l < 5; l++) tintLevels[l].forEach((t, i) => {
    const a = colourLevels[l][i * 4 + 3];
    if (a > 0) assert.ok(Math.abs(t / a - 200 / 255) < 0.03, `level ${l} texel ${i}`);
  });
});

test('counter has its own top and side tiles; plate rack uses plate_rack on every face', () => {
  const names = [];
  names[B.stone] = 'stone'; names[B.counter] = 'counter'; names[B.plate_rack] = 'plate_rack';
  const layer = (n) => { const i = NAMES.indexOf(n); assert.ok(i >= 0, n); return i; };
  const at = (b, dy) => tileFor(B, names, b, dy, false);
  assert.equal(at(B.counter, 1), layer('counter_top'));
  assert.equal(at(B.counter, 0), layer('counter_side'));
  assert.equal(at(B.counter, -1), layer('counter_side'));
  for (const dy of [1, 0, -1]) assert.equal(at(B.plate_rack, dy), layer('plate_rack'));
  assert.equal(at(B.stone, 0), layer('stone'));
});
