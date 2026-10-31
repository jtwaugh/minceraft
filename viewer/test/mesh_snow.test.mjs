import test from 'node:test';
import assert from 'node:assert/strict';
import { B, H, VERTEX_BYTES, LEAF_WEIGHT, meshBlocks } from '../mesh.js';

const ctx = { COLORS: [], faceLayer: new Uint8Array(256 * 3 * 2).fill(3) };
ctx.COLORS[B.spruce_leaves] = [60, 110, 70];
const tints = { grass: [[1, 1, 1]], leaf: [[0.5, 0.25, 0.1]] };
const snowOn = (under, fast = false) => {
  const blocks = new Uint8Array(H).fill(B.stone, 0, 41);
  blocks[41] = under; blocks[42] = B.snow_layer;
  return meshBlocks(ctx, blocks, tints, 1, 0, { greedy: false, fast });
};

test('snow on leaves or a bush is cut out with the leaf sway weight, so it sways with them', () => {
  for (const [under, fast] of [[B.oak_leaves, false], [B.spruce_leaves, false], [B.bush, true]]) {
    const m = snowOn(under, fast);
    assert.equal(m.cutout.faces, 11, 'five foliage faces and six of snow');
    const u8 = new Uint8Array(m.cutout.verts);
    for (let v = 0; v < m.cutout.faces * 4; v++) assert.equal(u8[v * VERTEX_BYTES + 14], LEAF_WEIGHT);
  }
});

test('snow on stone, or on fast leaves, stays opaque and still', () => {
  assert.equal(snowOn(B.stone).cutout.faces, 0);
  assert.equal(snowOn(B.oak_leaves, true).cutout.faces, 0);
});
